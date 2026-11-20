#![forbid(unsafe_code)]

mod check;

use std::error::Error;
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
            println!("{report}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("codeint: {}", describe(&error));
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
