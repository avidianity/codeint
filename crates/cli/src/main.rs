#![forbid(unsafe_code)]

mod check;
mod remedy;

use std::error::Error;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use codeint_config::{Command, ConfigError, ReportDestination, read_invocation};

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Check(#[from] check::CheckError),
    #[error("cannot write the report to standard output")]
    Output(#[source] std::io::Error),
    #[error("cannot write the report to {}", path.display())]
    OutputFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("codeint: {}\n{}", describe(&error), remedy::remedy(&error));
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), CliError> {
    match read_invocation()?.command {
        Command::Check {
            folder,
            destination,
            findings,
        } => write_report(&check::check(&folder, findings)?, &destination),
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
