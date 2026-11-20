#![forbid(unsafe_code)]

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read the current folder")]
    CurrentFolder(#[source] std::io::Error),
    #[error("{} does not exist", path.display())]
    MissingPath { path: PathBuf },
    #[error("{} is not a folder", path.display())]
    NotAFolder { path: PathBuf },
    #[error("cannot open {}", path.display())]
    UnreadablePath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Check { folder: PathBuf },
}

#[derive(Debug)]
pub struct Invocation {
    pub command: Command,
}

#[derive(Debug, Parser)]
#[command(
    name = "codeint",
    version,
    about = "Measures the Software entropy of a codebase"
)]
struct Arguments {
    #[command(subcommand)]
    command: CommandArguments,
}

#[derive(Debug, Subcommand)]
enum CommandArguments {
    #[command(about = "Measure the working tree and print the report")]
    Check {
        #[arg(
            help = "A folder inside the Git repository to measure; the current folder by default"
        )]
        path: Option<PathBuf>,
    },
}

pub fn read_invocation() -> Result<Invocation, ConfigError> {
    let command = match Arguments::parse().command {
        CommandArguments::Check { path } => Command::Check {
            folder: existing_folder(path)?,
        },
    };
    Ok(Invocation { command })
}

fn existing_folder(path: Option<PathBuf>) -> Result<PathBuf, ConfigError> {
    todo!("resolve {path:?} against the current folder and require an existing folder")
}
