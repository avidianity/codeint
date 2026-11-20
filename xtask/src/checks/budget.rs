use std::path::Path;
use std::process::Command;

use crate::config::Settings;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum FeedbackLoop {
    Hook,
    Relevant,
    CiGate,
}

impl FeedbackLoop {
    pub(crate) fn from_name(name: &str) -> Result<FeedbackLoop, String> {
        match name {
            "hook" => Ok(FeedbackLoop::Hook),
            "relevant" => Ok(FeedbackLoop::Relevant),
            "ci-gate" => Ok(FeedbackLoop::CiGate),
            other => Err(format!(
                "unknown Feedback loop {other}; use hook, relevant, or ci-gate"
            )),
        }
    }

    pub(crate) fn title(self) -> &'static str {
        match self {
            FeedbackLoop::Hook => "Git hook",
            FeedbackLoop::Relevant => "Relevant tier",
            FeedbackLoop::CiGate => "CI gate",
        }
    }

    pub(crate) fn budget_seconds(self, settings: &Settings) -> f64 {
        match self {
            FeedbackLoop::Hook => settings.budgets.hook_seconds,
            FeedbackLoop::Relevant => settings.budgets.relevant_seconds,
            FeedbackLoop::CiGate => settings.budgets.ci_gate_seconds,
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
pub(crate) enum Verdict {
    Within,
    NearBudget,
    OverBudget,
}

pub(crate) fn verdict(seconds: f64, budget: f64, warn_ratio: f64) -> Verdict {
    if seconds > budget {
        Verdict::OverBudget
    } else if seconds >= budget * warn_ratio {
        Verdict::NearBudget
    } else {
        Verdict::Within
    }
}

pub(crate) fn over_budget_message(
    feedback_loop: FeedbackLoop,
    seconds: f64,
    budget: f64,
) -> String {
    format!(
        "{} took {seconds:.0} seconds; its Feedback budget is {budget:.0} seconds. Make the loop faster in this pull request. Never raise the budget to pass; see docs/agents/testing.md.",
        feedback_loop.title()
    )
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

pub(crate) fn parse_timestamp(text: &str) -> Option<i64> {
    let text = text.strip_suffix('Z')?;
    let (date, time) = text.split_once('T')?;
    let date: Vec<i64> = date
        .split('-')
        .map(|part| part.parse().ok())
        .collect::<Option<_>>()?;
    let time: Vec<i64> = time
        .split(':')
        .map(|part| part.split('.').next().and_then(|whole| whole.parse().ok()))
        .collect::<Option<_>>()?;
    match (date.as_slice(), time.as_slice()) {
        ([year, month, day], [hour, minute, second]) => Some(
            days_from_civil(*year, *month, *day) * 86_400 + hour * 3_600 + minute * 60 + second,
        ),
        _ => None,
    }
}

pub(crate) fn ci_gate_seconds(jobs: &serde_json::Value, excluded_job: &str) -> Option<f64> {
    let jobs = jobs.get("jobs")?.as_array()?;
    let mut start: Option<i64> = None;
    let mut end: Option<i64> = None;
    for job in jobs {
        if job.get("name").and_then(serde_json::Value::as_str) == Some(excluded_job) {
            continue;
        }
        let started = job
            .get("started_at")
            .and_then(serde_json::Value::as_str)
            .and_then(parse_timestamp);
        let completed = job
            .get("completed_at")
            .and_then(serde_json::Value::as_str)
            .and_then(parse_timestamp);
        if let (Some(started), Some(completed)) = (started, completed) {
            start = Some(start.map_or(started, |current| current.min(started)));
            end = Some(end.map_or(completed, |current| current.max(completed)));
        }
    }
    Some((end? - start?) as f64)
}

pub(crate) fn fetch_jobs(root: &Path, run_id: &str) -> Result<serde_json::Value, String> {
    let output = gh(
        root,
        &[
            "api",
            &format!("repos/{{owner}}/{{repo}}/actions/runs/{run_id}/jobs?per_page=100"),
        ],
    )?;
    serde_json::from_str(&output)
        .map_err(|error| format!("cannot parse the jobs of run {run_id}: {error}"))
}

fn gh(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("gh")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot run gh: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "gh {} failed: {}",
            args.first().copied().unwrap_or_default(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub(crate) fn report_near_budget(
    root: &Path,
    feedback_loop: FeedbackLoop,
    seconds: f64,
    budget: f64,
) -> Result<(), String> {
    let title = format!("Feedback budget: {}", feedback_loop.title());
    let body = format!(
        "The {} loop took {seconds:.0} seconds, which is {:.0}% of its Feedback budget of {budget:.0} seconds.\nMake the loop faster before it reaches 100%; see docs/agents/testing.md.",
        feedback_loop.title(),
        seconds / budget * 100.0
    );
    let open = gh(
        root,
        &[
            "issue",
            "list",
            "--label",
            "budget",
            "--state",
            "open",
            "--json",
            "number,title",
        ],
    )?;
    let open: serde_json::Value = serde_json::from_str(&open)
        .map_err(|error| format!("cannot parse open budget issues: {error}"))?;
    let existing = open.as_array().and_then(|issues| {
        issues
            .iter()
            .find(|issue| {
                issue.get("title").and_then(serde_json::Value::as_str) == Some(title.as_str())
            })
            .and_then(|issue| issue.get("number").and_then(serde_json::Value::as_u64))
    });
    match existing {
        Some(number) => gh(
            root,
            &["issue", "comment", &number.to_string(), "--body", &body],
        )
        .map(|_| ()),
        None => gh(
            root,
            &[
                "issue", "create", "--title", &title, "--label", "budget", "--body", &body,
            ],
        )
        .map(|_| ()),
    }
}

#[cfg(test)]
mod tests;
