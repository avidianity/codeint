#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

mod calendar_day;
mod codeint_json;
mod regression_limits;
mod result_cache_folder;

pub use calendar_day::{CalendarDay, CalendarDayError};
pub use codeint_json::{
    Budget, Budgets, CONFIGURATION_FILE_NAME, Configuration, ConfigurationFileError,
    DetectedLanguage, JsonType, KeyPath, KeySegment, LanguageCoverage, Languages,
    PluginDeclaration, Sha256Digest, SourceSetOverrides, StarterConfigurationError,
    configuration_schema, published_schema_address, read_configuration,
    write_starter_configuration,
};
pub use regression_limits::{NewFindingsRule, RatioRise, RegressionLimit, RegressionLimits};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum TrendFormat {
    Toon,
    Json,
    Markdown,
}

const LATEST_COMMITS_OF_A_TREND: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrendRange {
    Latest(usize),
    Since(CalendarDay),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Check {
        folder: PathBuf,
        destination: ReportDestination,
        findings: FindingsShown,
        format: ReportFormat,
    },
    Diff {
        folder: PathBuf,
        base_ref: String,
        head_ref: Option<String>,
        destination: ReportDestination,
        findings: FindingsShown,
        format: ReportFormat,
        result_cache: Option<PathBuf>,
    },
    Trend {
        folder: PathBuf,
        range: TrendRange,
        destination: ReportDestination,
        format: TrendFormat,
        result_cache: Option<PathBuf>,
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
        about = "Compare the working tree, or a head Git ref, with the merge-base of a base Git ref and classify each Finding as new, resolved, or unchanged"
    )]
    Diff {
        #[arg(
            value_name = "BASE_REF",
            help = "The base Git ref, such as main or origin/main; codeint measures its merge-base with the head"
        )]
        base_ref: String,
        #[arg(
            long = "head",
            value_name = "REF",
            help = "Measure this Git ref from Git objects instead of the working tree; the merge-base is then taken with this ref instead of HEAD"
        )]
        head_ref: Option<String>,
        #[command(flatten)]
        report: ReportArguments,
    },
    #[command(
        about = "Measure each commit of the main line of the current branch, oldest first, and print each Metric over time"
    )]
    Trend {
        #[arg(
            long,
            value_name = "YYYY-MM-DD",
            help = "Measure every main-line commit committed on or after this day, in UTC, instead of the latest 20"
        )]
        since: Option<CalendarDay>,
        #[arg(
            long,
            value_name = "FILE",
            help = "Write the report to this file instead of standard output"
        )]
        output: Option<PathBuf>,
        #[arg(
            long,
            value_enum,
            default_value_t = TrendFormat::Toon,
            help = "The format of the report"
        )]
        format: TrendFormat,
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
        CommandArguments::Diff {
            base_ref,
            head_ref,
            report,
        } => Command::Diff {
            folder: current_folder.clone(),
            base_ref,
            head_ref,
            destination: report_destination(&current_folder, report.output)?,
            findings: findings_shown(report.all),
            format: report.format,
            result_cache: result_cache_folder_of_this_process(&current_folder),
        },
        CommandArguments::Trend {
            since,
            output,
            format,
        } => Command::Trend {
            folder: current_folder.clone(),
            range: since.map_or(
                TrendRange::Latest(LATEST_COMMITS_OF_A_TREND),
                TrendRange::Since,
            ),
            destination: report_destination(&current_folder, output)?,
            format,
            result_cache: result_cache_folder_of_this_process(&current_folder),
        },
        CommandArguments::Init => Command::Init {
            folder: current_folder,
        },
    };
    Ok(Invocation { command })
}

fn result_cache_folder_of_this_process(current_folder: &Path) -> Option<PathBuf> {
    result_cache_folder::result_cache_folder(
        current_folder,
        result_cache_folder::CacheFolderConvention::of_this_platform(),
        |name| std::env::var_os(name),
    )
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
