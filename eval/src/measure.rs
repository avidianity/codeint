use std::path::Path;
use std::process::Command;
use std::time::Instant;

use serde_json::Value;

use crate::command::describe;
use crate::error::{EvaluationError, io_error};

const TIME_PROGRAM: &str = "/usr/bin/time";

#[cfg(target_os = "macos")]
const TIME_RESOURCE_FLAG: &str = "-l";
#[cfg(not(target_os = "macos"))]
const TIME_RESOURCE_FLAG: &str = "-v";

pub(crate) struct CheckMeasurement {
    pub(crate) report: Value,
    pub(crate) exit_code: Option<i32>,
    pub(crate) wall_seconds: Vec<f64>,
    pub(crate) peak_rss_bytes: Option<u64>,
}

struct TimedRun {
    stdout: Vec<u8>,
    exit_code: Option<i32>,
    wall_seconds: f64,
    peak_rss_bytes: Option<u64>,
}

pub(crate) fn measure_check(
    codeint: &Path,
    checkout: &Path,
    scratch: &Path,
    runs: u32,
) -> Result<CheckMeasurement, EvaluationError> {
    let timed: Vec<TimedRun> = (0..runs.max(1))
        .map(|_| run_check_once(codeint, checkout, scratch))
        .collect::<Result<_, _>>()?;
    let first = &timed[0];
    let report =
        serde_json::from_slice(&first.stdout).map_err(|error| EvaluationError::Command {
            command: format!("{} check --format json --all", codeint.display()),
            detail: format!(
                "exit code {:?}, unreadable report: {error}",
                first.exit_code
            ),
        })?;
    Ok(CheckMeasurement {
        report,
        exit_code: first.exit_code,
        wall_seconds: timed.iter().map(|run| run.wall_seconds).collect(),
        peak_rss_bytes: timed.iter().filter_map(|run| run.peak_rss_bytes).max(),
    })
}

fn check_command(codeint: &Path, time_file: &Path) -> Command {
    let mut command = if Path::new(TIME_PROGRAM).exists() {
        let mut timed = Command::new(TIME_PROGRAM);
        timed
            .arg(TIME_RESOURCE_FLAG)
            .arg("-o")
            .arg(time_file)
            .arg(codeint);
        timed
    } else {
        Command::new(codeint)
    };
    command.args(["check", "--format", "json", "--all"]);
    command
}

fn run_check_once(
    codeint: &Path,
    checkout: &Path,
    scratch: &Path,
) -> Result<TimedRun, EvaluationError> {
    let time_file = scratch.join("time.txt");
    let mut command = check_command(codeint, &time_file);
    command.current_dir(checkout);
    let started = Instant::now();
    let output = command.output().map_err(|error| EvaluationError::Command {
        command: describe(&command),
        detail: error.to_string(),
    })?;
    let wall_seconds = started.elapsed().as_secs_f64();
    let time_text = std::fs::read_to_string(&time_file).unwrap_or_default();
    if time_file.exists() {
        std::fs::remove_file(&time_file).map_err(io_error(&time_file))?;
    }
    Ok(TimedRun {
        stdout: output.stdout,
        exit_code: output.status.code(),
        wall_seconds,
        peak_rss_bytes: parse_peak_rss(&time_text),
    })
}

pub(crate) fn parse_peak_rss(text: &str) -> Option<u64> {
    text.lines().map(str::trim).find_map(|line| {
        if let Some(kilobytes) = line.strip_prefix("Maximum resident set size (kbytes):") {
            return kilobytes
                .trim()
                .parse::<u64>()
                .ok()
                .map(|value| value * 1024);
        }
        line.strip_suffix("maximum resident set size")
            .and_then(|bytes| bytes.trim().parse().ok())
    })
}

pub(crate) fn median(values: &[f64]) -> Option<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    match sorted.len() {
        0 => None,
        length if length % 2 == 1 => Some(sorted[middle]),
        _ => Some((sorted[middle - 1] + sorted[middle]) / 2.0),
    }
}

#[cfg(test)]
mod tests;
