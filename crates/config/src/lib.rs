#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

mod boundaries;
mod calendar_day;
mod churn;
mod codeint_json;
mod codeowners;
mod config_error;
mod coupling;
mod regression_limits;
mod result_cache_folder;

pub use boundaries::{
    Boundaries, BoundaryDirective, BoundaryPreset, BoundaryRuleDecision, BoundarySourceSet,
};
pub use calendar_day::{CalendarDay, CalendarDayError};
pub use churn::{ChurnFormat, ChurnSince, ChurnSinceError, ChurnWindow, DEFAULT_CHURN_DAYS};
pub use codeint_json::{
    Budget, Budgets, Configuration, DetectedLanguage, EntryPoints, LanguageCoverage, Languages,
    PluginDeclaration, PluginKind, PluginLimits, PluginSource, Sha256Digest, SourceSetOverrides,
    StarterConfigurationError, configuration_schema, published_schema_address, read_configuration,
    write_starter_configuration,
};
pub use codeowners::{Codeowners, CodeownersError, OwnerFilter};
pub use config_error::{ConfigurationFileError, JsonType, KeyPath, KeySegment};
pub use coupling::{
    CouplingRatio, CouplingRatioError, CouplingSettings, DEFAULT_MAX_COMMIT_FILES,
    DEFAULT_MIN_CO_CHANGES, DEFAULT_MIN_COUPLING_RATIO,
};
pub use regression_limits::{NewFindingsRule, RatioRise, RegressionLimit, RegressionLimits};

const DEFAULT_PLUGIN_DOWNLOAD_TIMEOUT_SECONDS: u64 = 60;

const TEST_CA_CERT_VARIABLE: &str = "CODEINT_TEST_CA_CERT";
const RELEASE_BASE_URL_VARIABLE: &str = "CODEINT_RELEASE_BASE_URL";
const PLUGIN_CACHE_SUBDIR: &str = "plugins";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginHostConfig {
    pub cache_dir: Option<PathBuf>,
    pub ca_cert: Option<PathBuf>,
    pub download_timeout_seconds: u64,
}

impl Default for PluginHostConfig {
    fn default() -> PluginHostConfig {
        PluginHostConfig {
            cache_dir: None,
            ca_cert: None,
            download_timeout_seconds: DEFAULT_PLUGIN_DOWNLOAD_TIMEOUT_SECONDS,
        }
    }
}

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

impl Default for FindingsShown {
    fn default() -> FindingsShown {
        FindingsShown::WorstPerMetric(WORST_FINDINGS_PER_METRIC)
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum GuardFormat {
    Text,
    Toon,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ImpactFormat {
    Toon,
    Json,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum TraceFormat {
    Toon,
    Json,
    Text,
}

pub const DEFAULT_MAX_TRACE_PATHS: usize = 10;
const LATEST_COMMITS_OF_A_TREND: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrendRange {
    Latest(usize),
    Since(CalendarDay),
}

impl Default for TrendRange {
    fn default() -> TrendRange {
        TrendRange::Latest(LATEST_COMMITS_OF_A_TREND)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Check {
        folder: PathBuf,
        destination: ReportDestination,
        findings: FindingsShown,
        format: ReportFormat,
        package: Option<String>,
        owner: Option<String>,
        plugin_host: PluginHostConfig,
    },
    Diff {
        folder: PathBuf,
        base_ref: String,
        head_ref: Option<String>,
        destination: ReportDestination,
        findings: FindingsShown,
        format: ReportFormat,
        package: Option<String>,
        owner: Option<String>,
        result_cache: Option<PathBuf>,
        plugin_host: PluginHostConfig,
    },
    Trend {
        folder: PathBuf,
        range: TrendRange,
        destination: ReportDestination,
        format: TrendFormat,
        result_cache: Option<PathBuf>,
        plugin_host: PluginHostConfig,
    },
    Impact {
        folder: PathBuf,
        paths: Vec<String>,
        changed: Option<String>,
        depth: Option<usize>,
        package: Option<String>,
        destination: ReportDestination,
        format: ImpactFormat,
        plugin_host: PluginHostConfig,
    },
    Trace {
        folder: PathBuf,
        from: String,
        to: String,
        all: bool,
        max_paths: usize,
        package: Option<String>,
        destination: ReportDestination,
        format: TraceFormat,
        plugin_host: PluginHostConfig,
    },
    Init {
        folder: PathBuf,
    },
    Update {
        check_only: bool,
        version: Option<String>,
        release_base_url: Option<String>,
    },
    Mcp {
        folder: PathBuf,
        result_cache: Option<PathBuf>,
        plugin_host: PluginHostConfig,
    },
    Guard {
        folder: PathBuf,
        target: PathBuf,
        format: GuardFormat,
    },
    Churn {
        folder: PathBuf,
        window: ChurnWindow,
        destination: ReportDestination,
        findings: FindingsShown,
        format: ChurnFormat,
        plugin_host: PluginHostConfig,
    },
    Coupling {
        folder: PathBuf,
        settings: CouplingSettings,
        destination: ReportDestination,
        findings: FindingsShown,
        format: ChurnFormat,
        plugin_host: PluginHostConfig,
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
        #[command(flatten)]
        output: OutputArgument,
        #[arg(
            long,
            value_enum,
            default_value_t = TrendFormat::Toon,
            help = "The format of the report"
        )]
        format: TrendFormat,
    },
    #[command(about = "Find the shortest runtime import path between two files in the repository")]
    Trace(TraceArguments),
    #[command(
        about = "Compute the transitive reverse-dependency blast radius of one or more repository files"
    )]
    Impact(ImpactArguments),
    #[command(
        about = "Write a starter codeint.json at the root of the Git repository that contains the current folder"
    )]
    Init,
    #[command(about = "Update a standalone codeint binary in place from GitHub Releases")]
    Update {
        #[arg(
            long,
            help = "Check whether a newer release is available without modifying the binary"
        )]
        check: bool,
        #[arg(
            long,
            value_name = "TAG",
            help = "Install this release tag, such as v0.1.0, instead of the latest release"
        )]
        version: Option<String>,
    },
    #[command(
        about = "Run a local stdio Model Context Protocol (MCP) server exposing check, diff, trend, guard, impact, trace, churn, and coupling tools"
    )]
    Mcp,
    #[command(
        about = "Show the architectural boundary zone, allowed and denied target zones, public_files constraints, and allowed importers for a file path"
    )]
    Guard {
        #[arg(
            value_name = "PATH",
            help = "Path to an existing or planned file inside the Git repository"
        )]
        path: PathBuf,
        #[arg(
            long,
            value_enum,
            default_value_t = GuardFormat::Text,
            help = "The format of the output"
        )]
        format: GuardFormat,
    },
    #[command(
        about = "Rank eroded and duplicated files alongside their Git commit churn independently per Metric"
    )]
    Churn {
        #[command(flatten)]
        history: HistoryReportArguments,
    },
    #[command(
        about = "Detect pairs of files that repeatedly change together in the same commits and flag direct runtime imports"
    )]
    Coupling {
        #[command(flatten)]
        history: HistoryReportArguments,
        #[arg(
            long = "max-commit-files",
            value_name = "COUNT",
            default_value_t = std::num::NonZeroUsize::new(DEFAULT_MAX_COMMIT_FILES).expect("non-zero default"),
            help = "Exclude commits touching more than this many files from co-change pair accumulation"
        )]
        max_commit_files: std::num::NonZeroUsize,
        #[arg(
            long = "min-co-changes",
            value_name = "COUNT",
            default_value_t = std::num::NonZeroUsize::new(DEFAULT_MIN_CO_CHANGES).expect("non-zero default"),
            help = "Report only pairs that changed together in at least this many commits"
        )]
        min_co_changes: std::num::NonZeroUsize,
        #[arg(
            long = "min-ratio",
            value_name = "RATIO",
            default_value_t = CouplingRatio::default(),
            help = "Report only pairs whose coupling ratio is at least this value between 0 and 1"
        )]
        min_ratio: CouplingRatio,
        #[arg(
            long = "include-tests",
            help = "Include Test Source set files alongside Production files"
        )]
        include_tests: bool,
    },
}

#[derive(Debug, clap::Args)]
struct HistoryReportArguments {
    #[arg(
        long,
        value_name = "WINDOW",
        conflicts_with = "commits",
        help = "Lookback window in days (such as 90d) or a UTC date (YYYY-MM-DD); defaults to 90d"
    )]
    since: Option<ChurnSince>,
    #[arg(
        long,
        value_name = "COUNT",
        conflicts_with = "since",
        help = "Walk at most this many main-line commits from HEAD"
    )]
    commits: Option<std::num::NonZeroUsize>,
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
        default_value_t = ChurnFormat::Toon,
        help = "The format of the report"
    )]
    format: ChurnFormat,
}

#[derive(Debug, clap::Args)]
struct OutputArgument {
    #[arg(
        long,
        value_name = "FILE",
        help = "Write the report to this file instead of standard output"
    )]
    output: Option<PathBuf>,
}

#[derive(Debug, clap::Args)]
struct ReportArguments {
    #[arg(
        long,
        value_name = "PATH",
        help = "Scope measurement and output to this Workspace package"
    )]
    package: Option<String>,
    #[command(flatten)]
    output: OutputArgument,
    #[arg(long, help = "List every Finding instead of the 20 worst per Metric")]
    all: bool,
    #[arg(
        long,
        value_enum,
        default_value_t = ReportFormat::Toon,
        help = "The format of the report"
    )]
    format: ReportFormat,
    #[arg(
        long,
        value_name = "OWNER",
        help = "Include only Findings whose CODEOWNERS owner matches this pattern"
    )]
    owner: Option<String>,
}

#[derive(Debug, clap::Args)]
struct TraceArguments {
    #[arg(
        value_name = "FROM",
        help = "Repository-relative path of the starting module"
    )]
    from: String,
    #[arg(
        value_name = "TO",
        help = "Repository-relative path of the target module"
    )]
    to: String,
    #[arg(
        long,
        help = "List all shortest runtime import paths up to --max-paths"
    )]
    all: bool,
    #[arg(
        long = "max-paths",
        value_name = "COUNT",
        default_value_t = DEFAULT_MAX_TRACE_PATHS,
        help = "Maximum number of shortest paths to list when --all is set"
    )]
    max_paths: usize,
    #[arg(
        long,
        value_name = "PATH",
        help = "Scope the query to this Workspace package"
    )]
    package: Option<String>,
    #[command(flatten)]
    output: OutputArgument,
    #[arg(
        long,
        value_enum,
        default_value_t = TraceFormat::Toon,
        help = "The format of the report"
    )]
    format: TraceFormat,
}

#[derive(Debug, clap::Args)]
struct ImpactArguments {
    #[arg(
        value_name = "PATH",
        required_unless_present = "changed",
        help = "Repository file paths to analyze"
    )]
    paths: Vec<String>,
    #[arg(
        long,
        value_name = "BASE_REF",
        help = "Analyze files changed since the merge-base of this Git ref and HEAD"
    )]
    changed: Option<String>,
    #[arg(
        long,
        value_name = "N",
        help = "Maximum reverse-dependency hop distance to traverse"
    )]
    depth: Option<usize>,
    #[arg(
        long,
        value_name = "PATH",
        help = "Scope reported dependents to this Workspace package"
    )]
    package: Option<String>,
    #[command(flatten)]
    output: OutputArgument,
    #[arg(
        long,
        value_enum,
        default_value_t = ImpactFormat::Toon,
        help = "The format of the report"
    )]
    format: ImpactFormat,
}

pub fn read_invocation() -> Result<Invocation, ConfigError> {
    let arguments = Arguments::parse();
    let current_folder = std::env::current_dir().map_err(ConfigError::CurrentFolder)?;
    let command = build_command(&current_folder, arguments.command)?;
    Ok(Invocation { command })
}

fn build_command(
    current_folder: &Path,
    arguments: CommandArguments,
) -> Result<Command, ConfigError> {
    match arguments {
        CommandArguments::Check { path, report } => {
            build_check_command(current_folder, path, report)
        }
        CommandArguments::Diff {
            base_ref,
            head_ref,
            report,
        } => build_diff_command(current_folder, base_ref, head_ref, report),
        CommandArguments::Trend {
            since,
            output,
            format,
        } => build_trend_command(current_folder, since, output.output, format),
        CommandArguments::Impact(args) => build_impact_command(current_folder, args),
        CommandArguments::Trace(args) => build_trace_command(current_folder, args),
        other => build_auxiliary_command(current_folder, other),
    }
}

fn build_auxiliary_command(
    current_folder: &Path,
    arguments: CommandArguments,
) -> Result<Command, ConfigError> {
    match arguments {
        CommandArguments::Init => Ok(Command::Init {
            folder: current_folder.to_path_buf(),
        }),
        CommandArguments::Update { check, version } => Ok(build_update_command(check, version)),
        CommandArguments::Mcp => Ok(Command::Mcp {
            result_cache: result_cache_folder_of_this_process(current_folder),
            plugin_host: plugin_host_config_of_this_process(current_folder),
            folder: current_folder.to_path_buf(),
        }),
        CommandArguments::Guard { path, format } => Ok(Command::Guard {
            folder: current_folder.to_path_buf(),
            target: path,
            format,
        }),
        CommandArguments::Churn { history } => build_churn_command(current_folder, history),
        CommandArguments::Coupling {
            history,
            max_commit_files,
            min_co_changes,
            min_ratio,
            include_tests,
        } => build_coupling_command(
            current_folder,
            history,
            max_commit_files.get(),
            min_co_changes.get(),
            min_ratio,
            include_tests,
        ),
        _ => unreachable!("analysis commands are handled by build_command"),
    }
}

fn resolve_churn_window(
    since: Option<ChurnSince>,
    commits: Option<std::num::NonZeroUsize>,
) -> ChurnWindow {
    match (since, commits) {
        (Some(since), None) => ChurnWindow::from(since),
        (None, Some(count)) => ChurnWindow::Commits(count.get()),
        _ => ChurnWindow::default(),
    }
}

fn build_churn_command(
    current_folder: &Path,
    history: HistoryReportArguments,
) -> Result<Command, ConfigError> {
    Ok(Command::Churn {
        folder: current_folder.to_path_buf(),
        window: resolve_churn_window(history.since, history.commits),
        destination: report_destination(current_folder, history.output)?,
        findings: findings_shown(history.all),
        format: history.format,
        plugin_host: plugin_host_config_of_this_process(current_folder),
    })
}

fn build_coupling_command(
    current_folder: &Path,
    history: HistoryReportArguments,
    max_commit_files: usize,
    min_co_changes: usize,
    min_ratio: CouplingRatio,
    include_tests: bool,
) -> Result<Command, ConfigError> {
    Ok(Command::Coupling {
        folder: current_folder.to_path_buf(),
        settings: CouplingSettings {
            window: resolve_churn_window(history.since, history.commits),
            max_commit_files,
            min_co_changes,
            min_ratio,
            include_tests,
        },
        destination: report_destination(current_folder, history.output)?,
        findings: findings_shown(history.all),
        format: history.format,
        plugin_host: plugin_host_config_of_this_process(current_folder),
    })
}

fn build_check_command(
    current_folder: &Path,
    path: Option<PathBuf>,
    report: ReportArguments,
) -> Result<Command, ConfigError> {
    Ok(Command::Check {
        folder: existing_folder(current_folder, path)?,
        destination: report_destination(current_folder, report.output.output)?,
        findings: findings_shown(report.all),
        format: report.format,
        package: report.package,
        owner: report.owner,
        plugin_host: plugin_host_config_of_this_process(current_folder),
    })
}

fn build_diff_command(
    current_folder: &Path,
    base_ref: String,
    head_ref: Option<String>,
    report: ReportArguments,
) -> Result<Command, ConfigError> {
    Ok(Command::Diff {
        folder: current_folder.to_path_buf(),
        base_ref,
        head_ref,
        destination: report_destination(current_folder, report.output.output)?,
        findings: findings_shown(report.all),
        format: report.format,
        package: report.package,
        owner: report.owner,
        result_cache: result_cache_folder_of_this_process(current_folder),
        plugin_host: plugin_host_config_of_this_process(current_folder),
    })
}

fn build_trend_command(
    current_folder: &Path,
    since: Option<CalendarDay>,
    output: Option<PathBuf>,
    format: TrendFormat,
) -> Result<Command, ConfigError> {
    Ok(Command::Trend {
        folder: current_folder.to_path_buf(),
        range: since.map_or_else(TrendRange::default, TrendRange::Since),
        destination: report_destination(current_folder, output)?,
        format,
        result_cache: result_cache_folder_of_this_process(current_folder),
        plugin_host: plugin_host_config_of_this_process(current_folder),
    })
}

fn build_impact_command(
    current_folder: &Path,
    args: ImpactArguments,
) -> Result<Command, ConfigError> {
    Ok(Command::Impact {
        folder: current_folder.to_path_buf(),
        paths: args.paths,
        changed: args.changed,
        depth: args.depth,
        package: args.package,
        destination: report_destination(current_folder, args.output.output)?,
        format: args.format,
        plugin_host: plugin_host_config_of_this_process(current_folder),
    })
}

fn build_trace_command(
    current_folder: &Path,
    args: TraceArguments,
) -> Result<Command, ConfigError> {
    Ok(Command::Trace {
        folder: current_folder.to_path_buf(),
        from: args.from,
        to: args.to,
        all: args.all,
        max_paths: args.max_paths,
        package: args.package,
        destination: report_destination(current_folder, args.output.output)?,
        format: args.format,
        plugin_host: plugin_host_config_of_this_process(current_folder),
    })
}

fn build_update_command(check_only: bool, version: Option<String>) -> Command {
    Command::Update {
        check_only,
        version,
        release_base_url: std::env::var(RELEASE_BASE_URL_VARIABLE)
            .ok()
            .filter(|value| !value.is_empty()),
    }
}

fn result_cache_folder_of_this_process(current_folder: &Path) -> Option<PathBuf> {
    result_cache_folder::result_cache_folder(
        current_folder,
        result_cache_folder::CacheFolderConvention::of_this_platform(),
        |name| std::env::var_os(name),
    )
}

fn plugin_host_config_of_this_process(current_folder: &Path) -> PluginHostConfig {
    let cache_dir = result_cache_folder_of_this_process(current_folder)
        .map(|dir| dir.join(PLUGIN_CACHE_SUBDIR));
    let ca_cert = std::env::var_os(TEST_CA_CERT_VARIABLE)
        .filter(|value| !value.is_empty())
        .map(|value| current_folder.join(value));
    PluginHostConfig {
        cache_dir,
        ca_cert,
        download_timeout_seconds: DEFAULT_PLUGIN_DOWNLOAD_TIMEOUT_SECONDS,
    }
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
