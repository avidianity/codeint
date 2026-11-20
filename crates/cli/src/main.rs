#![forbid(unsafe_code)]

mod check;
mod diff;
mod init;
mod remedy;

use std::error::Error;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use codeint_config::{Command, ConfigError, ReportDestination, read_invocation};
use codeint_report::ReportError;

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Check(#[from] check::CheckError),
    #[error(transparent)]
    Init(#[from] init::InitError),
    #[error(transparent)]
    Report(#[from] ReportError),
    #[error("cannot write the report to standard output")]
    Output(#[source] std::io::Error),
    #[error("cannot write the report to {}", path.display())]
    OutputFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

const BUDGET_OR_REGRESSION_FAILED: u8 = 1;
const OPERATIONAL_ERROR: u8 = 2;

enum Outcome {
    Passed,
    Failed(Vec<String>),
}

impl Outcome {
    fn from_failures(failures: Vec<String>) -> Outcome {
        if failures.is_empty() {
            Outcome::Passed
        } else {
            Outcome::Failed(failures)
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(Outcome::Passed) => ExitCode::SUCCESS,
        Ok(Outcome::Failed(failures)) => {
            for failure in failures {
                eprintln!("codeint: {failure}");
            }
            ExitCode::from(BUDGET_OR_REGRESSION_FAILED)
        }
        Err(error) => {
            eprintln!("codeint: {}\n{}", describe(&error), remedy::remedy(&error));
            ExitCode::from(OPERATIONAL_ERROR)
        }
    }
}

fn run() -> Result<Outcome, CliError> {
    match read_invocation()?.command {
        Command::Check {
            folder,
            destination,
            findings,
            format,
        } => {
            let report = check::check(&folder, findings)?;
            write_report(&report.render(format)?, &destination)?;
            Ok(Outcome::from_failures(
                report
                    .over_budget()
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
            ))
        }
        Command::Diff {
            folder,
            base_ref,
            head_ref,
            destination,
            findings,
            format,
        } => {
            let refs = diff::Refs {
                base: &base_ref,
                head: head_ref.as_deref(),
            };
            let report = diff::diff(&folder, refs, findings)?;
            write_report(&report.render(format)?, &destination)?;
            Ok(Outcome::from_failures(report.failure_messages()))
        }
        Command::Init { folder } => {
            let written = init::init(&folder)?;
            print_report(&format!("Wrote {}\n", written.display()))?;
            Ok(Outcome::Passed)
        }
    }
}

fn write_report(report: &str, destination: &ReportDestination) -> Result<(), CliError> {
    let text = format!("{report}\n");
    match destination {
        ReportDestination::StandardOutput => print_report(&text),
        ReportDestination::File(path) => save_report(&text, path),
    }
}

fn print_report(text: &str) -> Result<(), CliError> {
    let mut output = std::io::stdout().lock();
    output
        .write_all(text.as_bytes())
        .and_then(|()| output.flush())
        .map_err(CliError::Output)
}

fn save_report(text: &str, path: &Path) -> Result<(), CliError> {
    std::fs::write(path, text).map_err(|source| CliError::OutputFile {
        path: path.to_path_buf(),
        source,
    })
}

fn describe(error: &dyn Error) -> String {
    let mut message = error.to_string();
    let mut cause = error.source();
    while let Some(source) = cause {
        message.push_str(": ");
        message.push_str(&source.to_string());
        cause = source.source();
    }
    message
}
