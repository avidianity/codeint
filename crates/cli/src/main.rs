#![forbid(unsafe_code)]

mod check;
mod remedy;

use std::error::Error;
use std::io::Write;
use std::process::ExitCode;

use codeint_config::{Command, ConfigError, read_invocation};

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Check(#[from] check::CheckError),
    #[error("cannot write the report to standard output")]
    Output(#[source] std::io::Error),
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
    let report = match read_invocation()?.command {
        Command::Check { folder } => check::check(&folder)?,
    };
    print_report(&report)
}

fn print_report(report: &str) -> Result<(), CliError> {
    let mut output = std::io::stdout().lock();
    writeln!(output, "{report}")
        .and_then(|()| output.flush())
        .map_err(CliError::Output)
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
