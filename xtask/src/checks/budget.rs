use std::collections::BTreeMap;
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

pub(crate) fn ci_gate_seconds(runs_jobs: &[serde_json::Value], excluded_job: &str) -> Option<f64> {
    let mut start: Option<i64> = None;
    let mut end: Option<i64> = None;
    for response in runs_jobs {
        let Some(jobs) = response.get("jobs").and_then(serde_json::Value::as_array) else {
            continue;
        };
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
    }
    Some((end? - start?) as f64)
}

pub(crate) fn pull_request_head_sha(run: &serde_json::Value) -> Option<&str> {
    if run.get("event").and_then(serde_json::Value::as_str) != Some("pull_request") {
        return None;
    }
    run.get("head_sha")
        .and_then(serde_json::Value::as_str)
        .filter(|sha| !sha.is_empty())
}

fn workflow_key(run: &serde_json::Value) -> Option<String> {
    run.get("workflow_id")
        .and_then(serde_json::Value::as_u64)
        .map(|id| id.to_string())
        .or_else(|| {
            run.get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
}

pub(crate) fn gating_run_ids(
    primary_run: &serde_json::Value,
    sha_runs: Option<&serde_json::Value>,
    primary_run_id: &str,
) -> Vec<String> {
    let mut run_ids = vec![primary_run_id.to_string()];
    if pull_request_head_sha(primary_run).is_none() {
        return run_ids;
    }
    let Some(runs) = sha_runs
        .and_then(|value| value.get("workflow_runs"))
        .and_then(serde_json::Value::as_array)
    else {
        return run_ids;
    };
    let primary_workflow = workflow_key(primary_run);
    let mut latest_by_workflow: BTreeMap<String, u64> = BTreeMap::new();
    for run in runs {
        if run.get("event").and_then(serde_json::Value::as_str) != Some("pull_request") {
            continue;
        }
        if run.get("conclusion").and_then(serde_json::Value::as_str) == Some("cancelled") {
            continue;
        }
        let Some(id) = run.get("id").and_then(serde_json::Value::as_u64) else {
            continue;
        };
        if id.to_string() == primary_run_id {
            continue;
        }
        let Some(key) = workflow_key(run) else {
            continue;
        };
        if primary_workflow.as_deref() == Some(key.as_str()) {
            continue;
        }
        latest_by_workflow
            .entry(key)
            .and_modify(|current| *current = (*current).max(id))
            .or_insert(id);
    }
    run_ids.extend(latest_by_workflow.into_values().map(|id| id.to_string()));
    run_ids
}

fn fetch_json(root: &Path, endpoint: &str, description: &str) -> Result<serde_json::Value, String> {
    let output = gh(root, &["api", endpoint])?;
    serde_json::from_str(&output).map_err(|error| format!("cannot parse {description}: {error}"))
}

pub(crate) fn fetch_ci_gate_jobs(
    root: &Path,
    run_id: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let primary_run = fetch_json(
        root,
        &format!("repos/{{owner}}/{{repo}}/actions/runs/{run_id}"),
        &format!("run {run_id}"),
    )?;
    let sha_runs = match pull_request_head_sha(&primary_run) {
        Some(head_sha) => Some(fetch_json(
            root,
            &format!(
                "repos/{{owner}}/{{repo}}/actions/runs?head_sha={head_sha}&event=pull_request&per_page=100"
            ),
            &format!("workflow runs for {head_sha}"),
        )?),
        None => None,
    };
    gating_run_ids(&primary_run, sha_runs.as_ref(), run_id)
        .iter()
        .map(|id| {
            fetch_json(
                root,
                &format!("repos/{{owner}}/{{repo}}/actions/runs/{id}/jobs?per_page=100"),
                &format!("the jobs of run {id}"),
            )
        })
        .collect()
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
