#![forbid(unsafe_code)]

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, thiserror::Error)]
#[error("cannot read the current directory")]
pub struct ConfigError(#[source] std::io::Error);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Subcommand)]
pub enum Command {
    #[command(about = "Measure the working tree and print the report")]
    Check,
}

#[derive(Debug)]
pub struct Invocation {
    pub command: Command,
    pub working_directory: PathBuf,
}

#[derive(Debug, Parser)]
#[command(
    name = "codeint",
    version,
    about = "Measures the Software entropy of a codebase"
)]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

pub fn read_invocation() -> Result<Invocation, ConfigError> {
    let arguments = Arguments::parse();
    let working_directory = std::env::current_dir().map_err(ConfigError)?;
    Ok(Invocation {
        command: arguments.command,
        working_directory,
    })
}
