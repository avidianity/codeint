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
    #[error("cannot write the report to {}: its folder does not exist", path.display())]
    MissingOutputFolder { path: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportDestination {
    StandardOutput,
    File(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Check {
        folder: PathBuf,
        destination: ReportDestination,
    },
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
        #[command(flatten)]
        report: ReportArguments,
    },
}

#[derive(Debug, clap::Args)]
struct ReportArguments {
    #[arg(
        long,
        value_name = "FILE",
        help = "Write the report to this file instead of standard output"
    )]
    output: Option<PathBuf>,
}

pub fn read_invocation() -> Result<Invocation, ConfigError> {
    let command = match Arguments::parse().command {
        CommandArguments::Check { path, report } => Command::Check {
            folder: existing_folder(path)?,
            destination: report_destination(report.output)?,
        },
    };
    Ok(Invocation { command })
}

fn existing_folder(path: Option<PathBuf>) -> Result<PathBuf, ConfigError> {
    let current_folder = std::env::current_dir().map_err(ConfigError::CurrentFolder)?;
    let Some(path) = path else {
        return Ok(current_folder);
    };
    let folder = std::fs::canonicalize(current_folder.join(&path)).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            ConfigError::MissingPath { path: path.clone() }
        } else {
            ConfigError::UnreadablePath {
                path: path.clone(),
                source,
            }
        }
    })?;
    if folder.is_dir() {
        Ok(folder)
    } else {
        Err(ConfigError::NotAFolder { path })
    }
}

fn report_destination(output: Option<PathBuf>) -> Result<ReportDestination, ConfigError> {
    todo!("{output:?}")
}
