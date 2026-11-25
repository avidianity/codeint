#![forbid(unsafe_code)]

mod config;

use std::process::ExitCode;

use codeint_eval::{EvaluationSummary, run_evaluation};

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
