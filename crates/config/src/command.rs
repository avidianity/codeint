use std::path::PathBuf;

use crate::OutputFormat;
use crate::PluginKind;
use crate::PluginLimits;
use crate::calendar_day::CalendarDay;
use crate::churn::{ChurnFormat, ChurnWindow};
use crate::coupling::CouplingSettings;
use crate::resolved::ConfigOverrides;

pub(crate) const DEFAULT_PLUGIN_DOWNLOAD_TIMEOUT_SECONDS: u64 = 60;
pub(crate) const WORST_FINDINGS_PER_METRIC: usize = 20;
pub const DEFAULT_MAX_TRACE_PATHS: usize = 10;
const LATEST_COMMITS_OF_A_TREND: usize = 20;

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
    #[error("cannot determine the home folder for agent configuration")]
    MissingAgentHome,
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

macro_rules! impl_full_output_format {
    ($target:ident) => {
        impl From<OutputFormat> for $target {
            fn from(format: OutputFormat) -> $target {
                match format {
                    OutputFormat::Toon => $target::Toon,
                    OutputFormat::Text => $target::Text,
                    OutputFormat::Json => $target::Json,
                    OutputFormat::Sarif => $target::Sarif,
                    OutputFormat::Markdown => $target::Markdown,
                }
            }
        }
    };
}

macro_rules! impl_subset_output_format {
    ($target:ident, $supported:expr, $($from:ident => $to:ident),+ $(,)?) => {
        impl $target {
            pub const SUPPORTED: &'static [&'static str] = $supported;

            pub fn try_from_output_format(format: OutputFormat) -> Option<$target> {
                match format {
                    $(OutputFormat::$from => Some($target::$to),)+
                    _ => None,
                }
            }
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ReportFormat {
    Toon,
    Text,
    Json,
    Sarif,
    Markdown,
}

impl_full_output_format!(ReportFormat);

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ReportRenderFormat {
    Toon,
    Text,
    Json,
    Sarif,
    Markdown,
}

impl_full_output_format!(ReportRenderFormat);

impl ReportRenderFormat {
    pub fn as_report_format(self) -> ReportFormat {
        match self {
            ReportRenderFormat::Toon => ReportFormat::Toon,
            ReportRenderFormat::Text => ReportFormat::Text,
            ReportRenderFormat::Json => ReportFormat::Json,
            ReportRenderFormat::Sarif => ReportFormat::Sarif,
            ReportRenderFormat::Markdown => ReportFormat::Markdown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum TrendFormat {
    Toon,
    Text,
    Json,
    Markdown,
}

impl_subset_output_format!(
    TrendFormat,
    &["toon", "text", "json", "markdown"],
    Toon => Toon,
    Text => Text,
    Json => Json,
    Markdown => Markdown,
);

impl_subset_output_format!(
    ChurnFormat,
    &["toon", "text", "json", "markdown"],
    Toon => Toon,
    Text => Text,
    Json => Json,
    Markdown => Markdown,
);

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
pub enum AgentFormat {
    Text,
    Toon,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum RatchetFormat {
    Text,
    Toon,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum BaselineFormat {
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

pub type PluginTestFormat = DoctorFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ImpactFormat {
    Toon,
    Json,
    Text,
}

impl_subset_output_format!(
    ImpactFormat,
    &["toon", "text", "json"],
    Toon => Toon,
    Text => Text,
    Json => Json,
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum TraceFormat {
    Toon,
    Json,
    Text,
}

impl_subset_output_format!(
    TraceFormat,
    &["toon", "text", "json"],
    Toon => Toon,
    Text => Text,
    Json => Json,
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum AgentClient {
    ClaudeCode,
    Cursor,
    Codex,
}

impl AgentClient {
    pub const ALL: [AgentClient; 3] = [
        AgentClient::ClaudeCode,
        AgentClient::Cursor,
        AgentClient::Codex,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            AgentClient::ClaudeCode => "claude-code",
            AgentClient::Cursor => "cursor",
            AgentClient::Codex => "codex",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            AgentClient::ClaudeCode => "Claude Code",
            AgentClient::Cursor => "Cursor",
            AgentClient::Codex => "Codex",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentCommand {
    Status {
        config_root: PathBuf,
        client: Option<AgentClient>,
        format: AgentFormat,
    },
    Install {
        config_root: PathBuf,
        client: Option<AgentClient>,
        dry_run: bool,
        format: AgentFormat,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum PluginCommand {
    Test {
        path: PathBuf,
        kind: PluginKind,
        sources: Vec<PathBuf>,
        limits: PluginLimits,
        format: PluginTestFormat,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RatchetCommand {
    Update {
        folder: PathBuf,
        reset: bool,
        coverage_reports: Vec<PathBuf>,
        format: RatchetFormat,
        plugin_host: PluginHostConfig,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaselineCommand {
    Update {
        folder: PathBuf,
        reset: bool,
        coverage_reports: Vec<PathBuf>,
        format: BaselineFormat,
        plugin_host: PluginHostConfig,
    },
}

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

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Check {
        folder: PathBuf,
        destination: ReportDestination,
        findings: FindingsShown,
        format: Option<ReportFormat>,
        package: Option<String>,
        owner: Option<String>,
        coverage_reports: Vec<PathBuf>,
        ignore_baseline: bool,
        plugin_host: PluginHostConfig,
    },
    Diff {
        folder: PathBuf,
        base_ref: String,
        head_ref: Option<String>,
        destination: ReportDestination,
        findings: FindingsShown,
        format: Option<ReportFormat>,
        package: Option<String>,
        owner: Option<String>,
        result_cache: Option<PathBuf>,
        plugin_host: PluginHostConfig,
    },
    Trend {
        folder: PathBuf,
        range: TrendRange,
        destination: ReportDestination,
        format: Option<TrendFormat>,
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
        format: Option<ImpactFormat>,
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
        format: Option<TraceFormat>,
        plugin_host: PluginHostConfig,
    },
    Init {
        folder: PathBuf,
        adopt: bool,
        plugin_host: PluginHostConfig,
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
        folder: PathBuf,
        input: ReportInput,
        destination: ReportDestination,
        all: bool,
        format: Option<ReportRenderFormat>,
    },
    Doctor {
        folder: PathBuf,
        format: DoctorFormat,
        plugin_host: PluginHostConfig,
    },
    Agent(AgentCommand),
    Ratchet(RatchetCommand),
    Plugin(PluginCommand),
    Baseline(BaselineCommand),
    Churn {
        folder: PathBuf,
        window: ChurnWindow,
        destination: ReportDestination,
        findings: FindingsShown,
        format: Option<ChurnFormat>,
        plugin_host: PluginHostConfig,
    },
    Coupling {
        folder: PathBuf,
        settings: CouplingSettings,
        destination: ReportDestination,
        findings: FindingsShown,
        format: Option<ChurnFormat>,
        plugin_host: PluginHostConfig,
    },
}

#[derive(Debug)]
pub struct Invocation {
    pub command: Command,
}
