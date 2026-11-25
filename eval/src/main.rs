#![forbid(unsafe_code)]

mod config;

use std::process::ExitCode;

use codeint_eval::{EvaluationSummary, run_evaluation, sweep_stale_scratch_folders};

fn print_results(summary: &EvaluationSummary) {
    for result in &summary.repositories {
        let verdict = if result.passed { "PASS" } else { "FAIL" };
        eprintln!(
            "{verdict} {} {} {}",
            result.name,
            result.commit,
            result.error.as_deref().unwrap_or_default()
        );
    }
}

fn main() -> ExitCode {
    let config = match config::evaluation_config() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("codeint-eval: cannot read the current folder: {error}");
            return ExitCode::from(2);
        }
    };
    let sweep = sweep_stale_scratch_folders(config.scratch_root.as_deref());
    eprintln!(
        "codeint-eval: removed {} stale scratch folders of ended runs",
        sweep.removed
    );
    for failure in &sweep.failures {
        eprintln!("codeint-eval: {failure}");
    }
    match run_evaluation(&config) {
        Ok(summary) => {
            print_results(&summary);
            ExitCode::from(u8::from(!summary.passed))
        }
        Err(error) => {
            eprintln!("codeint-eval: {error}");
            ExitCode::from(2)
        }
    }
}
