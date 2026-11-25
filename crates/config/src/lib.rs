#![forbid(unsafe_code)]

mod agent_home;
mod baseline;
mod boundaries;
mod calendar_day;
mod churn;
mod cli;
mod cli_schema;
mod codeint_json;
mod codeowners;
mod command;
mod config_error;
mod coupling;
mod dependencies;
mod framework_profiles;
mod hazards;
mod ratchet;
mod regression_limits;
mod repo_state;
mod resolved;
mod result_cache_folder;
mod sql;

pub use baseline::{
    BASELINE_PATH, BASELINE_SCHEMA_VERSION, Baseline, BaselineEntry, BaselineFileError,
    BaselineUpdateSummary, MeasuredBaselineMetric,
};
pub use boundaries::{
    Boundaries, BoundaryDirective, BoundaryExternalPolicy, BoundaryPreset, BoundaryRuleDecision,
    BoundarySourceSet,
};
pub use calendar_day::{CalendarDay, CalendarDayError};
pub use churn::{ChurnFormat, ChurnSince, ChurnSinceError, ChurnWindow, DEFAULT_CHURN_DAYS};
pub use cli::read_invocation;
pub use cli_schema::{
    CliArgumentSchema, CliCommandSchema, CliFlagSchema, CliSchema, OutputFormatSchema, cli_schema,
};
pub use codeint_json::{
    Budget, Budgets, Configuration, ConfigurationInspection, DEPENDENCY_METRIC_KEYS,
    DetectedLanguage, DiscoveredConfigFile, DuplicationConfig, EntryPoints, HAZARDS_METRIC_KEY,
    LanguageCoverage, Languages, MinSimilarity, PluginDeclaration, PluginKind, PluginLimits,
    PluginSource, SchemaValueSpan, SchemaVersionStatus, Sha256Digest, SourceSetOverrides,
    StarterBoundaries, StarterBoundaryZone, StarterConfigurationError, TEST_GAP_METRIC_KEY,
    configuration_schema, discover_config_file, inspect_configuration, is_broken_symlink,
    published_schema_address, read_configuration, read_resolved_configuration, schema_value_span,
    write_starter_configuration,
};
pub use codeowners::{Codeowners, CodeownersError, OwnerFilter};
pub use command::{
    AgentClient, AgentCommand, AgentFormat, BaselineCommand, BaselineFormat, Command, ConfigError,
    ConfigFormat, DEFAULT_MAX_TRACE_PATHS, DoctorFormat, FindingsShown, GuardFormat, ImpactFormat,
    Invocation, PluginCommand, PluginHostConfig, PluginTestFormat, RatchetCommand, RatchetFormat,
    ReportDestination, ReportFormat, ReportInput, ReportRenderFormat, SchemaFormat, TraceFormat,
    TrendFormat, TrendRange,
};
pub use config_error::{
    ConfigFileKind, ConfigurationFileError, JsonType, JsoncError, KeyPath, KeySegment,
};
pub use coupling::{
    CouplingRatio, CouplingRatioError, CouplingSettings, DEFAULT_MAX_COMMIT_FILES,
    DEFAULT_MIN_CO_CHANGES, DEFAULT_MIN_COUPLING_RATIO,
};
pub use dependencies::DependenciesConfig;
pub use framework_profiles::{FrameworkProfilesConfig, display_package_key};
pub use hazards::HazardsConfig;
pub use ratchet::{
    DEFAULT_RATCHET_PATH, MeasuredRatchetMetric, RATCHET_SCHEMA_VERSION, Ratchet, RatchetConfig,
    RatchetEntry, RatchetFileError, RatchetSourceSets, RatchetUpdateSummary,
    RatchetVersionMismatch, RatchetViolation, round_ratchet_ratio,
};
pub use regression_limits::{NewFindingsRule, RatioRise, RegressionLimit, RegressionLimits};
pub use repo_state::{
    LoadedForUpdate, MetricVersionMismatch, RepoStateFile, SourceSetEntries, StateFileError,
};
pub use resolved::{
    BudgetOverride, ConfigOverrides, ConfigProvenance, ResolvedConfiguration, ResolvedSetting,
    SourceSetOverride,
};
pub use sql::{KNOWN_SQL_DIALECTS, SqlConfig, SqlDialect, SqlDialectConflict};
