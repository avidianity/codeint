#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

mod codeint_json;

pub use codeint_json::{
    Budget, Budgets, CONFIGURATION_FILE_NAME, Configuration, ConfigurationFileError,
    DetectedLanguage, JsonType, KeyPath, KeySegment, LanguageCoverage, Languages,
    SourceSetOverrides, StarterConfigurationError, configuration_schema, published_schema_address,
    read_configuration, write_starter_configuration,
};

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
    #[error(
        "cannot write the report to {}: {} is not a folder that exists",
        path.display(),
        folder.display()
    )]
    MissingOutputFolder { path: PathBuf, folder: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportDestination {
    StandardOutput,
    File(PathBuf),
}

const WORST_FINDINGS_PER_METRIC: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingsShown {
    WorstPerMetric(usize),
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ReportFormat {
    Toon,
    Json,
    Sarif,
    Markdown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Check {
        folder: PathBuf,
        destination: ReportDestination,
        findings: FindingsShown,
        format: ReportFormat,
    },
    Init {
        folder: PathBuf,
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
    #[command(
        about = "Write a starter codeint.json at the root of the Git repository that contains the current folder"
    )]
    Init,
}

#[derive(Debug, clap::Args)]
struct ReportArguments {
    #[arg(
        long,
        value_name = "FILE",
        help = "Write the report to this file instead of standard output"
    )]
    output: Option<PathBuf>,
    #[arg(long, help = "List every Finding instead of the 20 worst per Metric")]
    all: bool,
    #[arg(
        long,
        value_enum,
        default_value_t = ReportFormat::Toon,
        help = "The format of the report"
    )]
    format: ReportFormat,
}

pub fn read_invocation() -> Result<Invocation, ConfigError> {
    let arguments = Arguments::parse();
    let current_folder = std::env::current_dir().map_err(ConfigError::CurrentFolder)?;
    let command = match arguments.command {
        CommandArguments::Check { path, report } => Command::Check {
            folder: existing_folder(&current_folder, path)?,
            destination: report_destination(&current_folder, report.output)?,
            findings: findings_shown(report.all),
            format: report.format,
        },
        CommandArguments::Init => Command::Init {
            folder: current_folder,
        },
    };
    Ok(Invocation { command })
}

fn existing_folder(current_folder: &Path, path: Option<PathBuf>) -> Result<PathBuf, ConfigError> {
    let Some(path) = path else {
        return Ok(current_folder.to_path_buf());
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

fn report_destination(
    current_folder: &Path,
    output: Option<PathBuf>,
) -> Result<ReportDestination, ConfigError> {
    let Some(path) = output else {
        return Ok(ReportDestination::StandardOutput);
    };
    let folder = path.parent().unwrap_or(Path::new("")).to_path_buf();
    if current_folder.join(&folder).is_dir() {
        Ok(ReportDestination::File(path))
    } else {
        Err(ConfigError::MissingOutputFolder { path, folder })
    }
}

fn findings_shown(all: bool) -> FindingsShown {
    if all {
        FindingsShown::All
    } else {
        FindingsShown::WorstPerMetric(WORST_FINDINGS_PER_METRIC)
    }
}
