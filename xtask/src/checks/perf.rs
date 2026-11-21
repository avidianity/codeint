pub(crate) mod corpus;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use crate::checks::Violations;
use crate::config::{Settings, temp_dir};

static NEXT_RUN_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PerfVerdict {
    Within,
    OverBudget,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MeasuredRun {
    pub(crate) files: usize,
    pub(crate) source_lines: u64,
    pub(crate) elapsed_seconds: f64,
}

struct TempCorpusDir {
    path: PathBuf,
}

impl TempCorpusDir {
    fn create() -> Result<TempCorpusDir, String> {
        let id = NEXT_RUN_ID.fetch_add(1, Ordering::Relaxed);
        let path = temp_dir().join(format!("codeint-perf-corpus-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path)
            .map_err(|error| format!("cannot create {}: {error}", path.display()))?;
        Ok(TempCorpusDir { path })
    }
}

impl Drop for TempCorpusDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub(crate) fn check_budget(elapsed_seconds: f64, budget_seconds: f64) -> PerfVerdict {
    if elapsed_seconds > budget_seconds {
        PerfVerdict::OverBudget
    } else {
        PerfVerdict::Within
    }
}

pub(crate) fn over_budget_message(run: MeasuredRun, budget_seconds: f64) -> String {
    format!(
        "Performance check took {elapsed:.2}s on {files} files ({lines} source lines); its budget in checks.toml is {budget_seconds:.2}s.",
        elapsed = run.elapsed_seconds,
        files = run.files,
        lines = run.source_lines,
    )
}

pub(crate) fn parse_report_totals(stdout: &[u8]) -> Result<(usize, u64), String> {
    let report: serde_json::Value = serde_json::from_slice(stdout)
        .map_err(|error| format!("cannot parse codeint JSON output: {error}"))?;
    let files = report
        .get("files")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or("codeint JSON output lacks integer files count")?;
    let source_lines = report
        .get("source_lines")
        .and_then(serde_json::Value::as_u64)
        .ok_or("codeint JSON output lacks integer source_lines count")?;
    Ok((files, source_lines))
}

fn default_binary_path(root: &Path) -> PathBuf {
    root.join("target")
        .join("release")
        .join(format!("codeint{}", std::env::consts::EXE_SUFFIX))
}

fn init_git_repo(dir: &Path) -> Result<(), String> {
    let steps: [&[&str]; 5] = [
        &["init", "-q"],
        &["config", "user.name", "codeint-perf"],
        &["config", "user.email", "perf@example.invalid"],
        &["add", "-A"],
        &["commit", "-q", "-m", "synthetic corpus"],
    ];
    for args in steps {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .map_err(|error| format!("cannot run git {}: {error}", args.join(" ")))?;
        if !output.status.success() {
            return Err(format!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
    }
    Ok(())
}

pub(crate) fn run_perf_check(
    root: &Path,
    settings: &Settings,
    binary_override: Option<&Path>,
) -> Result<Violations, String> {
    let binary = match binary_override {
        Some(path) => path.to_path_buf(),
        None => default_binary_path(root),
    };
    if !binary.is_file() {
        return Err(format!(
            "codeint binary not found at {}; build it first with `cargo build --release --locked --bin codeint`",
            binary.display()
        ));
    }

    let corpus_dir = TempCorpusDir::create()?;
    let spec = corpus::CorpusSpec::from_budget(settings.performance);
    let summary = corpus::write_corpus(&corpus_dir.path, spec)?;
    init_git_repo(&corpus_dir.path)?;

    let started = Instant::now();
    let output = Command::new(&binary)
        .args(["check", "--format", "json"])
        .current_dir(&corpus_dir.path)
        .output()
        .map_err(|error| format!("cannot run {}: {error}", binary.display()))?;
    let elapsed_seconds = started.elapsed().as_secs_f64();

    if !output.status.success() {
        return Err(format!(
            "codeint check exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let (files, source_lines) = parse_report_totals(&output.stdout)?;
    if files == 0 || source_lines == 0 {
        return Err(format!(
            "codeint measured empty output ({files} files, {source_lines} source lines) for generated corpus ({} files, {} lines)",
            summary.files, summary.lines
        ));
    }

    let budget_seconds = settings.performance.budget_seconds;
    println!(
        "Performance check: {files} files, {source_lines} source lines in {elapsed_seconds:.2}s (budget: {budget_seconds:.2}s, seed: {}).",
        spec.seed
    );

    let run = MeasuredRun {
        files,
        source_lines,
        elapsed_seconds,
    };
    Ok(match check_budget(elapsed_seconds, budget_seconds) {
        PerfVerdict::Within => Vec::new(),
        PerfVerdict::OverBudget => vec![over_budget_message(run, budget_seconds)],
    })
}

#[cfg(test)]
mod tests;
