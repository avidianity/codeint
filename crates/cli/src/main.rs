#![forbid(unsafe_code)]

mod check;

use std::process::ExitCode;

use codeint_config::{Command, ConfigError, read_invocation};

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Check(#[from] check::CheckError),
}

fn main() -> ExitCode {
    match run() {
        Ok(report) => {
            print!("{report}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("codeint: {error}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<String, CliError> {
    let invocation = read_invocation()?;
    match invocation.command {
        Command::Check => Ok(check::check(&invocation.working_directory)?),
    }
}
