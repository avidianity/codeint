#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

mod boundaries;
mod calendar_day;
mod cli_schema;
mod codeint_json;
mod config_error;
mod regression_limits;
mod resolved;
mod result_cache_folder;

pub use boundaries::{
    Boundaries, BoundaryDirective, BoundaryPreset, BoundaryRuleDecision, BoundarySourceSet,
};
pub use calendar_day::{CalendarDay, CalendarDayError};
pub use cli_schema::{
    CliArgumentSchema, CliCommandSchema, CliFlagSchema, CliSchema, OutputFormatSchema, cli_schema,
};
pub use codeint_json::{
    Budget, Budgets, Configuration, ConfigurationInspection, DetectedLanguage, LanguageCoverage,
    Languages, PluginDeclaration, PluginKind, PluginLimits, PluginSource, SchemaVersionStatus,
    Sha256Digest, SourceSetOverrides, StarterConfigurationError, configuration_schema,
    inspect_configuration, published_schema_address, read_configuration,
    read_resolved_configuration, write_starter_configuration,
};
pub use config_error::{ConfigurationFileError, JsonType, KeyPath, KeySegment};
pub use regression_limits::{NewFindingsRule, RatioRise, RegressionLimit, RegressionLimits};
pub use resolved::{
    BudgetOverride, ConfigOverrides, ConfigProvenance, ResolvedConfiguration, ResolvedSetting,
    SourceSetOverride,
};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportInput {
    StandardInput,
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
pub enum ReportRenderFormat {
    Toon,
    Json,
    Sarif,
    Markdown,
    Text,
}

impl ReportRenderFormat {
    pub fn as_report_format(self) -> ReportFormat {
        match self {
            ReportRenderFormat::Toon | ReportRenderFormat::Text => ReportFormat::Toon,
            ReportRenderFormat::Json => ReportFormat::Json,
            ReportRenderFormat::Sarif => ReportFormat::Sarif,
            ReportRenderFormat::Markdown => ReportFormat::Markdown,
        }
    }
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
pub enum SchemaFormat {
    Json,
    Toon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ConfigFormat {
    Text,
    Toon,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum DoctorFormat {
    Text,
    Json,
    Toon,
}

const LATEST_COMMITS_OF_A_TREND: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrendRange {
    Latest(usize),
    Since(CalendarDay),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Check {
        folder: PathBuf,
        destination: ReportDestination,
        findings: FindingsShown,
        format: ReportFormat,
        plugin_host: PluginHostConfig,
    },
    Diff {
        folder: PathBuf,
        base_ref: String,
        head_ref: Option<String>,
        destination: ReportDestination,
        findings: FindingsShown,
        format: ReportFormat,
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
    Schema {
        destination: ReportDestination,
        format: SchemaFormat,
    },
    Config {
        folder: PathBuf,
        format: ConfigFormat,
        overrides: ConfigOverrides,
    },
    Report {
        input: ReportInput,
        destination: ReportDestination,
        format: ReportRenderFormat,
    },
    Doctor {
        folder: PathBuf,
        format: DoctorFormat,
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
        about = "Run a local stdio Model Context Protocol (MCP) server exposing check, diff, trend, and guard tools"
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
        about = "Print a machine-readable capability manifest of CLI commands, flags, built-in Metrics, output formats, MCP tools, and exit codes"
    )]
    Schema {
        #[arg(
            long,
            value_name = "FILE",
            help = "Write the schema manifest to this file instead of standard output"
        )]
        output: Option<PathBuf>,
        #[arg(
            long,
            value_enum,
            default_value_t = SchemaFormat::Json,
            help = "The format of the schema manifest"
        )]
        format: SchemaFormat,
    },
    #[command(
        about = "Inspect the effective active configuration and where each setting came from"
    )]
    Config {
        #[arg(
            help = "A folder inside the Git repository to inspect; the current folder by default"
        )]
        path: Option<PathBuf>,
        #[arg(
            long,
            required = true,
            help = "Print the effective active configuration and per-field provenance"
        )]
        resolved: bool,
        #[arg(
            long,
            value_enum,
            default_value_t = ConfigFormat::Text,
            help = "The format of the output"
        )]
        format: ConfigFormat,
        #[arg(
            long = "budget",
            value_name = "SPEC",
            help = "Override a Budget ratio as SOURCE_SET.METRIC=RATIO"
        )]
        budgets: Vec<BudgetOverride>,
        #[arg(
            long = "source-set",
            value_name = "SPEC",
            help = "Override a Source set glob as SOURCE_SET=GLOB"
        )]
        source_sets: Vec<SourceSetOverride>,
        #[arg(
            long = "plugin-memory-mb",
            value_name = "MB",
            value_parser = clap::value_parser!(u64).range(1..=4096),
            help = "Override plugin_limits.memory_mb (1 to 4096)"
        )]
        plugin_memory_mb: Option<u64>,
        #[arg(
            long = "plugin-timeout-ms",
            value_name = "MS",
            value_parser = clap::value_parser!(u64).range(1..=300_000),
            help = "Override plugin_limits.timeout_ms (1 to 300000)"
        )]
        plugin_timeout_ms: Option<u64>,
    },
    #[command(
        about = "Re-render a saved check or diff JSON report into another format without re-running analysis"
    )]
    Report {
        #[arg(
            value_name = "INPUT",
            help = "Path to a saved check or diff JSON report, or - to read from standard input"
        )]
        input: PathBuf,
        #[arg(
            long,
            value_name = "FILE",
            help = "Write the report to this file instead of standard output"
        )]
        output: Option<PathBuf>,
        #[arg(
            long,
            value_enum,
            default_value_t = ReportRenderFormat::Toon,
            help = "The format of the report"
        )]
        format: ReportRenderFormat,
    },
    #[command(
        about = "Run read-only diagnostics on codeint.json, Git repository status, detected languages, Workspace packages, and external Wasm Plugins"
    )]
    Doctor {
        #[arg(help = "A folder to inspect; the current folder by default")]
        path: Option<PathBuf>,
        #[arg(
            long,
            value_enum,
            default_value_t = DoctorFormat::Text,
            help = "The format of the output"
        )]
        format: DoctorFormat,
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
        } => build_trend_command(current_folder, since, output, format),
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
        CommandArguments::Schema { output, format } => {
            build_schema_command(current_folder, output, format)
        }
        CommandArguments::Config {
            path,
            resolved: _,
            format,
            budgets,
            source_sets,
            plugin_memory_mb,
            plugin_timeout_ms,
        } => build_config_command(
            current_folder,
            path,
            format,
            ConfigOverrides {
                budgets,
                source_sets,
                plugin_memory_mb,
                plugin_timeout_ms,
            },
        ),
        CommandArguments::Report {
            input,
            output,
            format,
        } => build_report_command(current_folder, input, output, format),
        CommandArguments::Doctor { path, format } => Ok(Command::Doctor {
            folder: existing_folder(current_folder, path)?,
            format,
            plugin_host: plugin_host_config_of_this_process(current_folder),
        }),
    }
}

fn build_schema_command(
    current_folder: &Path,
    output: Option<PathBuf>,
    format: SchemaFormat,
) -> Result<Command, ConfigError> {
    Ok(Command::Schema {
        destination: report_destination(current_folder, output)?,
        format,
    })
}

fn build_report_command(
    current_folder: &Path,
    input: PathBuf,
    output: Option<PathBuf>,
    format: ReportRenderFormat,
) -> Result<Command, ConfigError> {
    let source = if input.as_os_str() == "-" {
        ReportInput::StandardInput
    } else {
        ReportInput::File(input)
    };
    Ok(Command::Report {
        input: source,
        destination: report_destination(current_folder, output)?,
        format,
    })
}

fn build_config_command(
    current_folder: &Path,
    path: Option<PathBuf>,
    format: ConfigFormat,
    overrides: ConfigOverrides,
) -> Result<Command, ConfigError> {
    Ok(Command::Config {
        folder: existing_folder(current_folder, path)?,
        format,
        overrides,
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
        range: since.map_or(
            TrendRange::Latest(LATEST_COMMITS_OF_A_TREND),
            TrendRange::Since,
        ),
        destination: report_destination(current_folder, output)?,
        format,
        result_cache: result_cache_folder_of_this_process(current_folder),
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
        destination: report_destination(current_folder, report.output)?,
        findings: findings_shown(report.all),
        format: report.format,
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
        destination: report_destination(current_folder, report.output)?,
        findings: findings_shown(report.all),
        format: report.format,
        result_cache: result_cache_folder_of_this_process(current_folder),
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
