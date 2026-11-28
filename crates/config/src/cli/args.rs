use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::PluginKind;
use crate::calendar_day::CalendarDay;
use crate::churn::{ChurnFormat, ChurnSince};
use crate::command::{
    AgentClient, AgentFormat, BaselineFormat, ConfigFormat, DEFAULT_MAX_TRACE_PATHS, DoctorFormat,
    GuardFormat, ImpactFormat, InitFormat, PluginTestFormat, RatchetFormat, ReportFormat,
    ReportRenderFormat, SchemaFormat, TraceFormat, TrendFormat,
};
use crate::config_file::{
    MAX_PLUGIN_MEMORY_MB, MAX_PLUGIN_TIMEOUT_MS, MIN_PLUGIN_MEMORY_MB, MIN_PLUGIN_TIMEOUT_MS,
};
use crate::coupling::{CouplingRatio, DEFAULT_MAX_COMMIT_FILES, DEFAULT_MIN_CO_CHANGES};
use crate::resolved::{BudgetOverride, SourceSetOverride};

#[derive(Debug, Parser)]
#[command(
    name = "codeint",
    version,
    about = "Measures the Software entropy of a codebase"
)]
pub(crate) struct Arguments {
    #[command(subcommand)]
    pub(super) command: CommandArguments,
}

#[derive(Debug, Subcommand)]
pub(super) enum CommandArguments {
    #[command(about = "Measure the working tree and print the report")]
    Check {
        #[arg(
            help = "A folder inside the Git repository to measure; the current folder by default"
        )]
        path: Option<PathBuf>,
        #[arg(
            long = "coverage-report",
            value_name = "PATH",
            help = "Path to an LCOV or Cobertura XML Coverage report; may repeat, and replaces coverage_reports in the configuration file"
        )]
        coverage_reports: Vec<PathBuf>,
        #[arg(
            long = "ignore-baseline",
            help = "Show every Finding and evaluate Budgets on every Finding, as if .codeint/baseline.json did not exist"
        )]
        ignore_baseline: bool,
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
            help = "The format of the report; overrides format in the configuration file (defaults to toon)"
        )]
        format: Option<TrendFormat>,
    },
    #[command(about = "Find the shortest runtime import path between two files in the repository")]
    Trace(TraceArguments),
    #[command(
        about = "Compute the transitive reverse-dependency blast radius of one or more repository files"
    )]
    Impact(ImpactArguments),
    #[command(
        about = "Write a starter configuration file at the root of the Git repository that contains the current folder"
    )]
    Init {
        #[arg(
            long,
            help = "Also enable per-file Regression checks, suggest new-file Budgets, write .codeint/ratchet.json and .codeint/baseline.json from the current state, and print pull-request CI instructions"
        )]
        adopt: bool,
        #[arg(
            long,
            value_enum,
            default_value_t = InitFormat::Jsonc,
            help = "The configuration file format to write"
        )]
        format: InitFormat,
    },
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
        about = "Print a machine-readable capability manifest of CLI commands, flags, built-in Metrics, output formats, MCP tools, and exit codes"
    )]
    Schema {
        #[command(flatten)]
        output: OutputArgument,
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
        #[command(flatten)]
        plugin_limits: PluginLimitArguments,
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
        #[command(flatten)]
        output: OutputArgument,
        #[arg(
            long,
            help = "List every Finding in text output instead of capping at 5 per Metric"
        )]
        all: bool,
        #[arg(
            long,
            value_enum,
            help = "The format of the report; overrides format in the configuration file (defaults to toon)"
        )]
        format: Option<ReportRenderFormat>,
    },
    #[command(
        about = "Run read-only diagnostics on the configuration file, Git repository status, detected languages, Workspace packages, and external Wasm Plugins"
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
    #[command(about = "Inspect and configure codeint MCP integration for local coding agents")]
    Agent {
        #[command(subcommand)]
        command: AgentSubcommandArguments,
    },
    #[command(about = "Check an external Wasm Plugin against the codeint Plugin contract")]
    Plugin {
        #[command(subcommand)]
        command: PluginSubcommandArguments,
    },
    #[command(about = "Manage the repository Ratchet ceilings in .codeint/ratchet.json")]
    Ratchet {
        #[command(subcommand)]
        command: RatchetSubcommandArguments,
    },
    #[command(
        about = "Manage the repository Baseline of existing Findings in .codeint/baseline.json"
    )]
    Baseline {
        #[command(subcommand)]
        command: BaselineSubcommandArguments,
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
pub(super) struct PluginLimitArguments {
    #[arg(
        long = "plugin-memory-mb",
        value_name = "MB",
        value_parser = clap::value_parser!(u64).range(MIN_PLUGIN_MEMORY_MB..=MAX_PLUGIN_MEMORY_MB),
        help = "Override the memory limit in MiB of each Plugin call, plugin_limits.memory_mb (1 to 4096)"
    )]
    pub(super) memory_mb: Option<u64>,
    #[arg(
        long = "plugin-timeout-ms",
        value_name = "MS",
        value_parser = clap::value_parser!(u64).range(MIN_PLUGIN_TIMEOUT_MS..=MAX_PLUGIN_TIMEOUT_MS),
        help = "Override the time limit in milliseconds of each Plugin call, plugin_limits.timeout_ms (1 to 300000)"
    )]
    pub(super) timeout_ms: Option<u64>,
}

#[derive(Debug, Subcommand)]
pub(super) enum AgentSubcommandArguments {
    #[command(
        about = "Report detected local coding agents and whether codeint mcp is registered for each"
    )]
    Status {
        #[arg(long, value_enum, help = "Inspect only this coding agent")]
        client: Option<AgentClient>,
        #[arg(
            long,
            value_enum,
            default_value_t = AgentFormat::Text,
            help = "The format of the output"
        )]
        format: AgentFormat,
        #[arg(
            long,
            value_name = "DIR",
            help = "Override the home folder containing agent configuration files"
        )]
        config_root: Option<PathBuf>,
    },
    #[command(about = "Register codeint mcp in supported local coding agent configuration files")]
    Install {
        #[arg(
            long,
            help = "Preview exact configuration changes without modifying any files"
        )]
        dry_run: bool,
        #[arg(
            long,
            value_enum,
            help = "Configure only this coding agent instead of all detected ones"
        )]
        client: Option<AgentClient>,
        #[arg(
            long,
            value_enum,
            default_value_t = AgentFormat::Text,
            help = "The format of the output"
        )]
        format: AgentFormat,
        #[arg(
            long,
            value_name = "DIR",
            help = "Override the home folder containing agent configuration files"
        )]
        config_root: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum PluginSubcommandArguments {
    #[command(
        about = "Run the Conformance runner against a Wasm Plugin and report each violated rule with a fix hint"
    )]
    Test {
        #[arg(value_name = "WASM", help = "Path to the Wasm Plugin to check")]
        path: PathBuf,
        #[arg(
            long,
            value_enum,
            default_value_t = PluginKind::Language,
            help = "The kind of Plugin to check"
        )]
        kind: PluginKind,
        #[arg(
            long = "source",
            value_name = "FILE",
            help = "A source file for a Language plugin to lower in addition to the built-in probe sources; may repeat"
        )]
        sources: Vec<PathBuf>,
        #[command(flatten)]
        plugin_limits: PluginLimitArguments,
        #[arg(
            long,
            value_enum,
            default_value_t = PluginTestFormat::Text,
            help = "The format of the output"
        )]
        format: PluginTestFormat,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum RatchetSubcommandArguments {
    #[command(
        about = "Create or monotonically tighten .codeint/ratchet.json from the current working tree"
    )]
    Update {
        #[arg(
            help = "A folder inside the Git repository to measure; the current folder by default"
        )]
        path: Option<PathBuf>,
        #[arg(
            long,
            help = "Allow loosening Ratchet ceilings for Metrics whose Metric version has not changed"
        )]
        reset: bool,
        #[arg(
            long = "coverage-report",
            value_name = "PATH",
            help = "Path to an LCOV or Cobertura XML Coverage report; may repeat, and replaces coverage_reports in the configuration file"
        )]
        coverage_reports: Vec<PathBuf>,
        #[arg(
            long,
            value_enum,
            default_value_t = RatchetFormat::Text,
            help = "The format of the output"
        )]
        format: RatchetFormat,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum BaselineSubcommandArguments {
    #[command(
        about = "Create .codeint/baseline.json, or remove fixed Findings from it, from the current working tree"
    )]
    Update {
        #[arg(
            help = "A folder inside the Git repository to measure; the current folder by default"
        )]
        path: Option<PathBuf>,
        #[arg(
            long,
            help = "Replace every Metric's Findings with the current Findings, including new ones"
        )]
        reset: bool,
        #[arg(
            long = "coverage-report",
            value_name = "PATH",
            help = "Path to an LCOV or Cobertura XML Coverage report; may repeat, and replaces coverage_reports in the configuration file"
        )]
        coverage_reports: Vec<PathBuf>,
        #[arg(
            long,
            value_enum,
            default_value_t = BaselineFormat::Text,
            help = "The format of the output"
        )]
        format: BaselineFormat,
    },
}

#[derive(Debug, clap::Args)]
pub(super) struct HistoryReportArguments {
    #[arg(
        long,
        value_name = "WINDOW",
        conflicts_with = "commits",
        help = "Lookback window in days (such as 90d) or a UTC date (YYYY-MM-DD); defaults to 90d"
    )]
    pub(super) since: Option<ChurnSince>,
    #[arg(
        long,
        value_name = "COUNT",
        conflicts_with = "since",
        help = "Walk at most this many main-line commits from HEAD"
    )]
    pub(super) commits: Option<std::num::NonZeroUsize>,
    #[command(flatten)]
    pub(super) output: OutputArgument,
    #[arg(long, help = "List every Finding instead of the 20 worst per Metric")]
    pub(super) all: bool,
    #[arg(
        long,
        value_enum,
        help = "The format of the report; overrides format in the configuration file (defaults to toon)"
    )]
    pub(super) format: Option<ChurnFormat>,
}

#[derive(Debug, clap::Args)]
pub(super) struct OutputArgument {
    #[arg(
        long,
        value_name = "FILE",
        help = "Write the report to this file instead of standard output"
    )]
    pub(super) output: Option<PathBuf>,
}

#[derive(Debug, clap::Args)]
pub(super) struct ReportArguments {
    #[arg(
        long,
        value_name = "PATH",
        help = "Scope measurement and output to this Workspace package"
    )]
    pub(super) package: Option<String>,
    #[command(flatten)]
    pub(super) output: OutputArgument,
    #[arg(long, help = "List every Finding instead of the 20 worst per Metric")]
    pub(super) all: bool,
    #[arg(
        long,
        value_enum,
        help = "The format of the report; overrides format in the configuration file (defaults to toon)"
    )]
    pub(super) format: Option<ReportFormat>,
    #[arg(
        long,
        value_name = "OWNER",
        help = "Include only Findings whose CODEOWNERS owner matches this pattern"
    )]
    pub(super) owner: Option<String>,
}

#[derive(Debug, clap::Args)]
pub(super) struct TraceArguments {
    #[arg(
        value_name = "FROM",
        help = "Repository-relative path of the starting module"
    )]
    pub(super) from: String,
    #[arg(
        value_name = "TO",
        help = "Repository-relative path of the target module"
    )]
    pub(super) to: String,
    #[arg(
        long,
        help = "List all shortest runtime import paths up to --max-paths"
    )]
    pub(super) all: bool,
    #[arg(
        long = "max-paths",
        value_name = "COUNT",
        default_value_t = DEFAULT_MAX_TRACE_PATHS,
        help = "Maximum number of shortest paths to list when --all is set"
    )]
    pub(super) max_paths: usize,
    #[arg(
        long,
        value_name = "PATH",
        help = "Scope the query to this Workspace package"
    )]
    pub(super) package: Option<String>,
    #[command(flatten)]
    pub(super) output: OutputArgument,
    #[arg(
        long,
        value_enum,
        help = "The format of the report; overrides format in the configuration file (defaults to toon)"
    )]
    pub(super) format: Option<TraceFormat>,
}

#[derive(Debug, clap::Args)]
pub(super) struct ImpactArguments {
    #[arg(
        value_name = "PATH",
        required_unless_present = "changed",
        help = "Repository file paths to analyze"
    )]
    pub(super) paths: Vec<String>,
    #[arg(
        long,
        value_name = "BASE_REF",
        help = "Analyze files changed since the merge-base of this Git ref and HEAD"
    )]
    pub(super) changed: Option<String>,
    #[arg(
        long,
        value_name = "N",
        help = "Maximum reverse-dependency hop distance to traverse"
    )]
    pub(super) depth: Option<usize>,
    #[arg(
        long,
        value_name = "PATH",
        help = "Scope reported dependents to this Workspace package"
    )]
    pub(super) package: Option<String>,
    #[command(flatten)]
    pub(super) output: OutputArgument,
    #[arg(
        long,
        value_enum,
        help = "The format of the report; overrides format in the configuration file (defaults to toon)"
    )]
    pub(super) format: Option<ImpactFormat>,
}
