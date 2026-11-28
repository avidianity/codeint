#![forbid(unsafe_code)]

mod agent;
mod baseline;
mod check;
mod churn;
mod commit_measurement;
mod config;
mod coupling;
mod diff;
mod doctor;
mod guard;
mod impact;
mod init;
mod mcp;
mod plugin_test;
mod ratchet;
mod remedy;
mod report;
mod trace;
mod trend;
mod update;

use std::error::Error;
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use codeint_config::{
    AgentCommand, BaselineCommand, Command, ConfigError, ConfigFileKind, DEFAULT_MAX_TRACE_PATHS,
    MAX_PLUGIN_MEMORY_MB, MAX_PLUGIN_TIMEOUT_MS, MIN_PLUGIN_MEMORY_MB, MIN_PLUGIN_TIMEOUT_MS,
    OutputFormat, PluginCommand, RatchetCommand, ReportDestination, ReportInput, cli_schema,
    read_configuration, read_invocation,
};
use codeint_git::{GitError, Repository, ResultCache};
use codeint_report::{ExitCodeSchema, ReportError};

pub use crate::init::AdoptedFiles;
pub use codeint_config::{
    AgentClient, AgentFormat, BaselineFormat, BaselineUpdateSummary, BudgetOverride, CalendarDay,
    ChurnFormat, ChurnSince, ChurnWindow, ConfigFormat, ConfigOverrides, CouplingRatio,
    CouplingRatioError, CouplingSettings, DoctorFormat, FindingsShown, GuardFormat, ImpactFormat,
    InitFormat, PluginHostConfig, PluginKind, PluginLimits, PluginTestFormat, RatchetFormat,
    ReportFormat, ReportRenderFormat, SchemaFormat, SourceSetOverride, TraceFormat, TrendFormat,
    TrendRange,
};
pub use codeint_report::{
    AgentInstallAction, AgentInstallEntry, AgentInstallReport, AgentStatusEntry, AgentStatusReport,
    ChurnReport, ConfigReport, CouplingReport, DiffReport, DoctorCheck, DoctorReport, DoctorStatus,
    DoctorSummary, GuardReport, ImpactReport, PluginTestCheck, PluginTestReport, PluginTestStatus,
    PluginTestSummary, RatchetFailure, RatchetUpdateReport, Report, ReportSchemaKind, SavedReport,
    SchemaReport, TraceReport, TrendReport, all_report_schemas, render_baseline_update,
    report_schema, schema_for,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CheckOptions {
    pub findings: FindingsShown,
    pub package: Option<String>,
    pub owner: Option<String>,
    pub coverage_reports: Vec<PathBuf>,
    pub ignore_baseline: bool,
    pub plugin_host: PluginHostConfig,
}

impl CheckOptions {
    fn scope(&self) -> check::CheckScope<'_> {
        check::CheckScope {
            findings: self.findings,
            package: self.package.as_deref(),
            owner: self.owner.as_deref(),
            coverage_reports: &self.coverage_reports,
            ignore_baseline: self.ignore_baseline,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiffOptions {
    pub head_ref: Option<String>,
    pub findings: FindingsShown,
    pub package: Option<String>,
    pub owner: Option<String>,
    pub result_cache: Option<PathBuf>,
    pub plugin_host: PluginHostConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrendOptions {
    pub range: TrendRange,
    pub result_cache: Option<PathBuf>,
    pub plugin_host: PluginHostConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImpactOptions {
    pub paths: Vec<String>,
    pub changed: Option<String>,
    pub depth: Option<usize>,
    pub package: Option<String>,
    pub plugin_host: PluginHostConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceOptions {
    pub all: bool,
    pub max_paths: usize,
    pub package: Option<String>,
    pub plugin_host: PluginHostConfig,
}

impl Default for TraceOptions {
    fn default() -> TraceOptions {
        TraceOptions {
            all: false,
            max_paths: DEFAULT_MAX_TRACE_PATHS,
            package: None,
            plugin_host: PluginHostConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChurnOptions {
    pub window: ChurnWindow,
    pub findings: FindingsShown,
    pub plugin_host: PluginHostConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CouplingOptions {
    pub settings: CouplingSettings,
    pub findings: FindingsShown,
    pub plugin_host: PluginHostConfig,
}
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InitOptions {
    pub format: InitFormat,
    pub adopt: bool,
    pub plugin_host: PluginHostConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitOutcome {
    pub configuration: PathBuf,
    pub adopted: Option<AdoptedFiles>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StateUpdateOptions {
    pub reset: bool,
    pub coverage_reports: Vec<PathBuf>,
    pub plugin_host: PluginHostConfig,
}

pub type RatchetUpdateOptions = StateUpdateOptions;
pub type BaselineUpdateOptions = StateUpdateOptions;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RerenderReportOptions {
    pub format: Option<ReportRenderFormat>,
    pub all: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PluginTestOptions {
    pub kind: PluginKind,
    pub sources: Vec<PathBuf>,
    pub limits: PluginLimits,
}

impl PluginTestOptions {
    pub fn new(kind: PluginKind) -> PluginTestOptions {
        PluginTestOptions {
            kind,
            sources: Vec::new(),
            limits: PluginLimits::default(),
        }
    }
}

pub struct GatedOutcome<T> {
    pub passed: bool,
    pub failures: Vec<String>,
    pub report: T,
}

impl<T> GatedOutcome<T> {
    fn from_report(report: T, failures: Vec<String>) -> GatedOutcome<T> {
        let passed = failures.is_empty();
        GatedOutcome {
            passed,
            failures,
            report,
        }
    }
}

pub type CheckOutcome = GatedOutcome<Report>;
pub type DiffOutcome = GatedOutcome<DiffReport>;
pub type DoctorOutcome = GatedOutcome<DoctorReport>;
pub type PluginTestOutcome = GatedOutcome<PluginTestReport>;

#[derive(Debug)]
pub struct CodeintError {
    message: String,
    remedy: String,
    mcp_remedy: String,
}

impl CodeintError {
    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn remedy(&self) -> &str {
        &self.remedy
    }

    pub(crate) fn mcp_text(&self) -> String {
        format!("{}\n{}", self.message, self.mcp_remedy)
    }

    pub(crate) fn from_cli_error(error: CliError) -> CodeintError {
        CodeintError {
            message: describe(&error),
            remedy: remedy::remedy(&error).into_owned(),
            mcp_remedy: remedy::mcp_remedy(&error).into_owned(),
        }
    }

    fn invalid_option(message: impl Into<String>, remedy: impl Into<String>) -> CodeintError {
        let remedy = remedy.into();
        CodeintError {
            message: message.into(),
            mcp_remedy: remedy.clone(),
            remedy,
        }
    }
}

impl fmt::Display for CodeintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\n{}", self.message, self.remedy)
    }
}

impl Error for CodeintError {}

pub fn check(folder: &Path, options: &CheckOptions) -> Result<CheckOutcome, CodeintError> {
    let report = check::check(folder, options.scope(), &options.plugin_host)
        .map_err(|err| CodeintError::from_cli_error(CliError::Check(err)))?;
    let failures = report.failure_messages();
    Ok(GatedOutcome::from_report(report, failures))
}

pub fn diff(
    folder: &Path,
    base_ref: &str,
    options: &DiffOptions,
) -> Result<DiffOutcome, CodeintError> {
    let report = measure_diff(folder, base_ref, options)
        .map_err(|err| CodeintError::from_cli_error(CliError::Check(err)))?;
    let failures = report.failure_messages();
    Ok(GatedOutcome::from_report(report, failures))
}

fn measure_diff(
    folder: &Path,
    base_ref: &str,
    options: &DiffOptions,
) -> Result<DiffReport, check::CheckError> {
    let refs = diff::Refs {
        base: base_ref,
        head: options.head_ref.as_deref(),
    };
    let cache = options.result_cache.clone().map(ResultCache::in_folder);
    diff::diff(
        folder,
        refs,
        options.findings,
        options.package.as_deref(),
        options.owner.as_deref(),
        cache.as_ref(),
        &options.plugin_host,
    )
}

pub fn trend(folder: &Path, options: &TrendOptions) -> Result<TrendReport, CodeintError> {
    validate_trend_range(options.range)?;
    let cache = options.result_cache.clone().map(ResultCache::in_folder);
    trend::trend(folder, options.range, cache.as_ref(), &options.plugin_host)
        .map_err(|err| CodeintError::from_cli_error(CliError::Check(err)))
}

pub fn impact(folder: &Path, options: &ImpactOptions) -> Result<ImpactReport, CodeintError> {
    let request = impact::ImpactRequest {
        paths: &options.paths,
        changed: options.changed.as_deref(),
        depth: options.depth,
        package: options.package.as_deref(),
        plugin_host: &options.plugin_host,
    };
    impact::impact(folder, &request)
        .map_err(|err| CodeintError::from_cli_error(CliError::Impact(err)))
}

pub fn trace(
    folder: &Path,
    from: &str,
    to: &str,
    options: &TraceOptions,
) -> Result<TraceReport, CodeintError> {
    trace::trace(
        folder,
        from,
        to,
        options.all,
        options.max_paths,
        options.package.as_deref(),
        &options.plugin_host,
    )
    .map_err(|err| CodeintError::from_cli_error(CliError::Trace(err)))
}

pub fn init(folder: &Path, options: &InitOptions) -> Result<InitOutcome, CodeintError> {
    if options.adopt {
        let adopted = init::adopt(folder, options.format, &options.plugin_host)
            .map_err(|err| CodeintError::from_cli_error(CliError::Init(err)))?;
        return Ok(InitOutcome {
            configuration: adopted.configuration.clone(),
            adopted: Some(adopted),
        });
    }
    let configuration = init::init(folder, options.format)
        .map_err(|err| CodeintError::from_cli_error(CliError::Init(err)))?;
    Ok(InitOutcome {
        configuration,
        adopted: None,
    })
}

pub fn guard(folder: &Path, target: &Path) -> Result<GuardReport, CodeintError> {
    guard::guard(folder, target).map_err(|err| CodeintError::from_cli_error(CliError::Guard(err)))
}

pub fn schema() -> SchemaReport {
    SchemaReport::build(
        cli_schema(),
        check::built_in_languages(),
        mcp::tool_definitions(),
        exit_codes(),
    )
    .with_language_hazard_rules(codeint_plugin_host::built_in_language_hazard_rules())
    .with_language_framework_profiles(codeint_plugin_host::built_in_language_framework_profiles())
}

pub fn resolved_config(
    folder: &Path,
    overrides: &ConfigOverrides,
) -> Result<ConfigReport, CodeintError> {
    validate_config_overrides(overrides)?;
    config::resolved_config(folder, overrides)
        .map_err(|err| CodeintError::from_cli_error(CliError::ConfigCommand(err)))
}

fn require_existing_folder(folder: &Path) -> Result<(), CodeintError> {
    let metadata = std::fs::metadata(folder).map_err(|source| {
        let error = if source.kind() == std::io::ErrorKind::NotFound {
            ConfigError::MissingPath {
                path: folder.to_path_buf(),
            }
        } else {
            ConfigError::UnreadablePath {
                path: folder.to_path_buf(),
                source,
            }
        };
        CodeintError::from_cli_error(CliError::Config(error))
    })?;
    if metadata.is_dir() {
        Ok(())
    } else {
        Err(CodeintError::from_cli_error(CliError::Config(
            ConfigError::NotAFolder {
                path: folder.to_path_buf(),
            },
        )))
    }
}

pub fn doctor(
    folder: &Path,
    plugin_host: &PluginHostConfig,
) -> Result<DoctorOutcome, CodeintError> {
    require_existing_folder(folder)?;
    let report = doctor::doctor(folder, plugin_host);
    let failures = report.blocking_error_messages();
    Ok(GatedOutcome::from_report(report, failures))
}

pub fn agent_status(
    config_root: &Path,
    client: Option<AgentClient>,
) -> Result<AgentStatusReport, CodeintError> {
    agent::status(config_root, client)
        .map_err(|err| CodeintError::from_cli_error(CliError::Agent(err)))
}

pub fn agent_install(
    config_root: &Path,
    client: Option<AgentClient>,
    dry_run: bool,
) -> Result<AgentInstallReport, CodeintError> {
    agent::install(config_root, client, dry_run)
        .map_err(|err| CodeintError::from_cli_error(CliError::Agent(err)))
}

pub fn churn(folder: &Path, options: &ChurnOptions) -> Result<ChurnReport, CodeintError> {
    validate_churn_window(options.window)?;
    churn::churn(
        folder,
        options.window,
        options.findings,
        &options.plugin_host,
    )
    .map_err(|err| CodeintError::from_cli_error(CliError::Check(err)))
}

pub fn coupling(folder: &Path, options: &CouplingOptions) -> Result<CouplingReport, CodeintError> {
    validate_coupling_settings(options.settings)?;
    coupling::coupling(
        folder,
        options.settings,
        options.findings,
        &options.plugin_host,
    )
    .map_err(|err| CodeintError::from_cli_error(CliError::Check(err)))
}

fn validate_trend_range(range: TrendRange) -> Result<(), CodeintError> {
    if matches!(range, TrendRange::Latest(0)) {
        return Err(CodeintError::invalid_option(
            "trend range must include at least 1 commit",
            "Pass TrendRange::Latest with a positive commit count (at least 1) or TrendRange::Since.",
        ));
    }
    Ok(())
}

fn validate_churn_window(window: ChurnWindow) -> Result<(), CodeintError> {
    match window {
        ChurnWindow::Days(0) => Err(CodeintError::invalid_option(
            "history window days must be at least 1",
            "Pass ChurnWindow::Days with a positive day count (at least 1).",
        )),
        ChurnWindow::Commits(0) => Err(CodeintError::invalid_option(
            "history window commits must be at least 1",
            "Pass ChurnWindow::Commits with a positive commit count (at least 1).",
        )),
        ChurnWindow::Days(_) | ChurnWindow::Commits(_) | ChurnWindow::Since(_) => Ok(()),
    }
}

fn validate_coupling_settings(settings: CouplingSettings) -> Result<(), CodeintError> {
    validate_churn_window(settings.window)?;
    if settings.max_commit_files == 0 {
        return Err(CodeintError::invalid_option(
            "coupling max_commit_files must be at least 1",
            "Set CouplingSettings::max_commit_files to a positive integer (at least 1).",
        ));
    }
    if settings.min_co_changes == 0 {
        return Err(CodeintError::invalid_option(
            "coupling min_co_changes must be at least 1",
            "Set CouplingSettings::min_co_changes to a positive integer (at least 1).",
        ));
    }
    Ok(())
}

fn validate_config_overrides(overrides: &ConfigOverrides) -> Result<(), CodeintError> {
    if let Some(memory_mb) = overrides.plugin_memory_mb
        && !(MIN_PLUGIN_MEMORY_MB..=MAX_PLUGIN_MEMORY_MB).contains(&memory_mb)
    {
        return Err(CodeintError::invalid_option(
            format!(
                "plugin_memory_mb {memory_mb} is out of range ({MIN_PLUGIN_MEMORY_MB}..={MAX_PLUGIN_MEMORY_MB})"
            ),
            format!(
                "Set ConfigOverrides::plugin_memory_mb to a value from {MIN_PLUGIN_MEMORY_MB} to {MAX_PLUGIN_MEMORY_MB} MiB."
            ),
        ));
    }
    if let Some(timeout_ms) = overrides.plugin_timeout_ms
        && !(MIN_PLUGIN_TIMEOUT_MS..=MAX_PLUGIN_TIMEOUT_MS).contains(&timeout_ms)
    {
        return Err(CodeintError::invalid_option(
            format!(
                "plugin_timeout_ms {timeout_ms} is out of range ({MIN_PLUGIN_TIMEOUT_MS}..={MAX_PLUGIN_TIMEOUT_MS})"
            ),
            format!(
                "Set ConfigOverrides::plugin_timeout_ms to a value from {MIN_PLUGIN_TIMEOUT_MS} to {MAX_PLUGIN_TIMEOUT_MS} milliseconds."
            ),
        ));
    }
    Ok(())
}

pub fn ratchet_update(
    folder: &Path,
    options: &RatchetUpdateOptions,
) -> Result<RatchetUpdateReport, CodeintError> {
    let (context, measured) =
        check::load_and_measure_whole_repo(folder, &options.coverage_reports, &options.plugin_host)
            .map_err(|err| CodeintError::from_cli_error(CliError::Check(err)))?;
    ratchet::update_from_report(&context, &measured, options.reset)
        .map_err(|err| CodeintError::from_cli_error(CliError::Ratchet(err)))
}

pub fn baseline_update(
    folder: &Path,
    options: &BaselineUpdateOptions,
) -> Result<BaselineUpdateSummary, CodeintError> {
    let (context, measured) =
        check::load_and_measure_whole_repo(folder, &options.coverage_reports, &options.plugin_host)
            .map_err(|err| CodeintError::from_cli_error(CliError::Check(err)))?;
    baseline::update_from_report(&context, &measured, options.reset)
        .map_err(|err| CodeintError::from_cli_error(CliError::Check(err)))
}

pub fn rerender_report(
    folder: &Path,
    input: &Path,
    options: &RerenderReportOptions,
) -> Result<String, CodeintError> {
    let format =
        resolve_report_format(folder, options.format).map_err(CodeintError::from_cli_error)?;
    let source = ReportInput::File(input.to_path_buf());
    report::rerender(&source, format, options.all)
        .map_err(|err| CodeintError::from_cli_error(CliError::Rerender(err)))
}

pub fn plugin_test(
    path: &Path,
    options: &PluginTestOptions,
) -> Result<PluginTestOutcome, CodeintError> {
    let report = plugin_test::plugin_test(path, options.kind, &options.sources, options.limits)
        .map_err(|err| CodeintError::from_cli_error(CliError::PluginTest(err)))?;
    let failures = report.failure_messages();
    Ok(GatedOutcome::from_report(report, failures))
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum CliError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    ConfigCommand(#[from] config::ConfigCommandError),
    #[error(transparent)]
    Check(#[from] check::CheckError),
    #[error(transparent)]
    Guard(#[from] guard::GuardError),
    #[error(transparent)]
    Agent(#[from] agent::AgentError),
    #[error(transparent)]
    Ratchet(#[from] ratchet::RatchetError),
    #[error(transparent)]
    PluginTest(#[from] plugin_test::PluginTestError),
    #[error(transparent)]
    Impact(#[from] impact::ImpactCommandError),
    #[error(transparent)]
    Trace(#[from] trace::TraceCommandError),
    #[error(transparent)]
    Init(#[from] init::InitError),
    #[error(transparent)]
    Update(#[from] update::UpdateError),
    #[error(transparent)]
    Report(#[from] ReportError),
    #[error(transparent)]
    Rerender(#[from] report::RerenderError),
    #[error(
        "codeint {command} does not support configured format \"{format}\" in {config_file}; supported formats for codeint {command}: {}",
        supported.join(", ")
    )]
    UnsupportedConfiguredFormat {
        command: &'static str,
        format: OutputFormat,
        config_file: ConfigFileKind,
        supported: &'static [&'static str],
    },
    #[error("cannot write the report to standard output")]
    Output(#[source] std::io::Error),
    #[error("cannot write the report to {}", path.display())]
    OutputFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

const SUCCESS: u8 = 0;
const BUDGET_OR_REGRESSION_FAILED: u8 = 1;
const OPERATIONAL_ERROR: u8 = 2;

fn exit_codes() -> Vec<ExitCodeSchema> {
    vec![
        ExitCodeSchema {
            code: SUCCESS,
            name: "success",
            description: "The run succeeded, and no Budget, Ratchet, Regression, codeint doctor blocking error, or codeint plugin test check failed.",
        },
        ExitCodeSchema {
            code: BUDGET_OR_REGRESSION_FAILED,
            name: "budget_or_regression_failed",
            description: "The run succeeded, and at least one Budget, Ratchet, Regression, codeint doctor blocking error, or codeint plugin test check failed.",
        },
        ExitCodeSchema {
            code: OPERATIONAL_ERROR,
            name: "operational_error",
            description: "An operational error stopped the run, such as an invalid option, missing Git ref, or invalid configuration file.",
        },
    ]
}

enum Outcome {
    Passed,
    Failed(Vec<String>),
}

impl Outcome {
    fn from_failures(failures: Vec<String>) -> Outcome {
        if failures.is_empty() {
            Outcome::Passed
        } else {
            Outcome::Failed(failures)
        }
    }
}

pub fn run_cli() -> ExitCode {
    match run() {
        Ok(Outcome::Passed) => ExitCode::SUCCESS,
        Ok(Outcome::Failed(failures)) => {
            for failure in failures {
                eprintln!("codeint: {failure}");
            }
            ExitCode::from(BUDGET_OR_REGRESSION_FAILED)
        }
        Err(error) => {
            eprintln!("codeint: {}\n{}", describe(&error), remedy::remedy(&error));
            ExitCode::from(OPERATIONAL_ERROR)
        }
    }
}

fn run() -> Result<Outcome, CliError> {
    run_command(read_invocation()?.command)
}

fn run_command(command: Command) -> Result<Outcome, CliError> {
    match command {
        Command::Check {
            folder,
            destination,
            findings,
            format,
            package,
            owner,
            coverage_reports,
            ignore_baseline,
            plugin_host,
        } => {
            let options = CheckOptions {
                findings,
                package,
                owner,
                coverage_reports,
                ignore_baseline,
                plugin_host,
            };
            run_check(&folder, &destination, format, &options)
        }
        Command::Diff {
            folder,
            base_ref,
            head_ref,
            destination,
            findings,
            format,
            package,
            owner,
            result_cache,
            plugin_host,
        } => {
            let options = DiffOptions {
                head_ref,
                findings,
                package,
                owner,
                result_cache,
                plugin_host,
            };
            run_diff(&folder, &base_ref, &destination, format, &options)
        }
        Command::Trend {
            folder,
            range,
            destination,
            format,
            result_cache,
            plugin_host,
        } => run_trend(
            &folder,
            range,
            &destination,
            format,
            result_cache,
            &plugin_host,
        ),
        Command::Impact {
            folder,
            paths,
            changed,
            depth,
            package,
            destination,
            format,
            plugin_host,
        } => {
            let request = impact::ImpactRequest {
                paths: &paths,
                changed: changed.as_deref(),
                depth,
                package: package.as_deref(),
                plugin_host: &plugin_host,
            };
            run_impact(&folder, &request, &destination, format)
        }
        Command::Trace {
            folder,
            from,
            to,
            all,
            max_paths,
            package,
            destination,
            format,
            plugin_host,
        } => {
            let options = TraceOptions {
                all,
                max_paths,
                package,
                plugin_host,
            };
            run_trace(&folder, &from, &to, &destination, format, &options)
        }
        other => run_auxiliary_command(other),
    }
}

fn run_auxiliary_command(command: Command) -> Result<Outcome, CliError> {
    match command {
        Command::Init {
            folder,
            adopt,
            format,
            plugin_host,
        } => run_init(&folder, adopt, format, &plugin_host),
        Command::Update {
            check_only,
            version,
            release_base_url,
        } => run_update(check_only, version.as_deref(), release_base_url.as_deref()),
        other => run_tooling_command(other),
    }
}

fn run_tooling_command(command: Command) -> Result<Outcome, CliError> {
    match command {
        Command::Mcp {
            folder,
            result_cache,
            plugin_host,
        } => run_mcp(&folder, result_cache, plugin_host),
        Command::Guard {
            folder,
            target,
            format,
        } => run_guard(&folder, &target, format),
        Command::Schema {
            destination,
            format,
        } => run_schema(&destination, format),
        Command::Config {
            folder,
            format,
            overrides,
        } => run_config(&folder, format, &overrides),
        Command::Report {
            folder,
            input,
            destination,
            all,
            format,
        } => run_report(&folder, &input, &destination, all, format),
        Command::Doctor {
            folder,
            format,
            plugin_host,
        } => run_doctor(&folder, format, &plugin_host),
        other => run_subcommand_group(other),
    }
}

fn run_subcommand_group(command: Command) -> Result<Outcome, CliError> {
    match command {
        Command::Agent(command) => run_agent(command),
        Command::Ratchet(command) => run_ratchet(command),
        Command::Plugin(command) => run_plugin(command),
        Command::Baseline(command) => run_baseline(command),
        other => run_history_command(other),
    }
}

fn run_history_command(command: Command) -> Result<Outcome, CliError> {
    match command {
        Command::Churn {
            folder,
            window,
            destination,
            findings,
            format,
            plugin_host,
        } => run_churn(
            &folder,
            window,
            &destination,
            findings,
            format,
            &plugin_host,
        ),
        Command::Coupling {
            folder,
            settings,
            destination,
            findings,
            format,
            plugin_host,
        } => run_coupling(
            &folder,
            settings,
            &destination,
            findings,
            format,
            &plugin_host,
        ),
        _ => unreachable!("handled by run"),
    }
}

fn run_plugin(command: PluginCommand) -> Result<Outcome, CliError> {
    match command {
        PluginCommand::Test {
            path,
            kind,
            sources,
            limits,
            format,
        } => {
            let report = plugin_test::plugin_test(&path, kind, &sources, limits)?;
            print_report(&format!("{}\n", report.render(format)?))?;
            Ok(Outcome::from_failures(report.failure_messages()))
        }
    }
}

fn run_agent(command: AgentCommand) -> Result<Outcome, CliError> {
    match command {
        AgentCommand::Status {
            config_root,
            client,
            format,
        } => {
            let report = agent::status(&config_root, client)?;
            print_report(&format!("{}\n", report.render(format)?))?;
            Ok(Outcome::Passed)
        }
        AgentCommand::Install {
            config_root,
            client,
            dry_run,
            format,
        } => {
            let report = agent::install(&config_root, client, dry_run)?;
            print_report(&format!("{}\n", report.render(format)?))?;
            Ok(Outcome::Passed)
        }
    }
}

fn run_ratchet(command: RatchetCommand) -> Result<Outcome, CliError> {
    match command {
        RatchetCommand::Update {
            folder,
            reset,
            coverage_reports,
            format,
            plugin_host,
        } => {
            let (context, measured) =
                check::load_and_measure_whole_repo(&folder, &coverage_reports, &plugin_host)?;
            let report = ratchet::update_from_report(&context, &measured, reset)?;
            print_report(&format!("{}\n", report.render(format)?))?;
            Ok(Outcome::Passed)
        }
    }
}

fn run_baseline(command: BaselineCommand) -> Result<Outcome, CliError> {
    match command {
        BaselineCommand::Update {
            folder,
            reset,
            coverage_reports,
            format,
            plugin_host,
        } => {
            let (context, measured) =
                check::load_and_measure_whole_repo(&folder, &coverage_reports, &plugin_host)?;
            let summary = baseline::update_from_report(&context, &measured, reset)?;
            print_report(&format!("{}\n", render_baseline_update(&summary, format)?))?;
            Ok(Outcome::Passed)
        }
    }
}

fn run_schema(destination: &ReportDestination, format: SchemaFormat) -> Result<Outcome, CliError> {
    let report = schema();
    write_report(&report.render(format)?, destination)?;
    Ok(Outcome::Passed)
}

fn run_config(
    folder: &Path,
    format: ConfigFormat,
    overrides: &ConfigOverrides,
) -> Result<Outcome, CliError> {
    let report = config::resolved_config(folder, overrides)?;
    print_report(&format!("{}\n", report.render(format)?))?;
    Ok(Outcome::Passed)
}

fn resolve_repo_format<F>(
    folder: &Path,
    explicit: Option<F>,
    command: &'static str,
    supported: &'static [&'static str],
    convert: fn(OutputFormat) -> Option<F>,
) -> Result<F, CliError> {
    if let Some(format) = explicit {
        return Ok(format);
    }
    let repository = Repository::discover(folder).map_err(check::CheckError::Git)?;
    let configuration =
        read_configuration(repository.root()).map_err(check::CheckError::Configuration)?;
    convert(configuration.format).ok_or(CliError::UnsupportedConfiguredFormat {
        command,
        format: configuration.format,
        config_file: configuration.config_file(),
        supported,
    })
}

fn resolve_report_format(
    folder: &Path,
    explicit: Option<ReportRenderFormat>,
) -> Result<ReportRenderFormat, CliError> {
    if let Some(format) = explicit {
        return Ok(format);
    }
    match Repository::discover(folder) {
        Ok(repository) => {
            let configuration =
                read_configuration(repository.root()).map_err(check::CheckError::Configuration)?;
            Ok(ReportRenderFormat::from(configuration.format))
        }
        Err(GitError::NotInRepository { .. } | GitError::BareRepository { .. }) => {
            Ok(ReportRenderFormat::Toon)
        }
        Err(error) => Err(CliError::Check(check::CheckError::Git(error))),
    }
}

fn run_report(
    folder: &Path,
    input: &ReportInput,
    destination: &ReportDestination,
    all: bool,
    format: Option<ReportRenderFormat>,
) -> Result<Outcome, CliError> {
    let format = resolve_report_format(folder, format)?;
    let rendered = report::rerender(input, format, all)?;
    write_report(&rendered, destination)?;
    Ok(Outcome::Passed)
}

fn run_doctor(
    folder: &Path,
    format: DoctorFormat,
    plugin_host: &PluginHostConfig,
) -> Result<Outcome, CliError> {
    let report = doctor::doctor(folder, plugin_host);
    print_report(&format!("{}\n", report.render(format)?))?;
    Ok(Outcome::from_failures(report.blocking_error_messages()))
}

fn run_churn(
    folder: &Path,
    window: ChurnWindow,
    destination: &ReportDestination,
    findings: FindingsShown,
    format: Option<ChurnFormat>,
    plugin_host: &PluginHostConfig,
) -> Result<Outcome, CliError> {
    let format = resolve_repo_format(
        folder,
        format,
        "churn",
        ChurnFormat::SUPPORTED,
        ChurnFormat::try_from_output_format,
    )?;
    let report = churn::churn(folder, window, findings, plugin_host)?;
    write_report(&report.render(format)?, destination)?;
    Ok(Outcome::Passed)
}

fn run_coupling(
    folder: &Path,
    settings: CouplingSettings,
    destination: &ReportDestination,
    findings: FindingsShown,
    format: Option<ChurnFormat>,
    plugin_host: &PluginHostConfig,
) -> Result<Outcome, CliError> {
    let format = resolve_repo_format(
        folder,
        format,
        "coupling",
        ChurnFormat::SUPPORTED,
        ChurnFormat::try_from_output_format,
    )?;
    let report = coupling::coupling(folder, settings, findings, plugin_host)?;
    write_report(&report.render(format)?, destination)?;
    Ok(Outcome::Passed)
}

fn run_guard(folder: &Path, target: &Path, format: GuardFormat) -> Result<Outcome, CliError> {
    let report = guard::guard(folder, target)?;
    print_report(&format!("{}\n", report.render(format)?))?;
    Ok(Outcome::Passed)
}

fn run_init(
    folder: &Path,
    adopt: bool,
    format: InitFormat,
    plugin_host: &PluginHostConfig,
) -> Result<Outcome, CliError> {
    if adopt {
        let adopted = init::adopt(folder, format, plugin_host)?;
        print_report(&init::render_adoption(&adopted))?;
        return Ok(Outcome::Passed);
    }
    let written = init::init(folder, format)?;
    print_report(&format!("Wrote {}\n", written.display()))?;
    Ok(Outcome::Passed)
}

fn run_update(
    check_only: bool,
    version: Option<&str>,
    release_base_url: Option<&str>,
) -> Result<Outcome, CliError> {
    let message = update::update(check_only, version, release_base_url)?;
    print_report(&format!("{message}\n"))?;
    Ok(Outcome::Passed)
}

fn run_mcp(
    folder: &Path,
    result_cache: Option<PathBuf>,
    plugin_host: PluginHostConfig,
) -> Result<Outcome, CliError> {
    mcp::serve_stdio(folder, result_cache, plugin_host).map_err(CliError::Output)?;
    Ok(Outcome::Passed)
}

const REPORT_FORMATS: [&str; 5] = ["toon", "text", "json", "sarif", "markdown"];

fn run_check(
    folder: &Path,
    destination: &ReportDestination,
    format: Option<ReportFormat>,
    options: &CheckOptions,
) -> Result<Outcome, CliError> {
    let format = resolve_repo_format(folder, format, "check", &REPORT_FORMATS, |configured| {
        Some(ReportFormat::from(configured))
    })?;
    let report = check::check(folder, options.scope(), &options.plugin_host)?;
    write_report(&report.render(format)?, destination)?;
    Ok(Outcome::from_failures(report.failure_messages()))
}

fn run_diff(
    folder: &Path,
    base_ref: &str,
    destination: &ReportDestination,
    format: Option<ReportFormat>,
    options: &DiffOptions,
) -> Result<Outcome, CliError> {
    let format = resolve_repo_format(folder, format, "diff", &REPORT_FORMATS, |configured| {
        Some(ReportFormat::from(configured))
    })?;
    let report = measure_diff(folder, base_ref, options)?;
    write_report(&report.render(format)?, destination)?;
    Ok(Outcome::from_failures(report.failure_messages()))
}

fn run_trend(
    folder: &Path,
    range: TrendRange,
    destination: &ReportDestination,
    format: Option<TrendFormat>,
    result_cache: Option<PathBuf>,
    plugin_host: &PluginHostConfig,
) -> Result<Outcome, CliError> {
    let format = resolve_repo_format(
        folder,
        format,
        "trend",
        TrendFormat::SUPPORTED,
        TrendFormat::try_from_output_format,
    )?;
    let result_cache = result_cache.map(ResultCache::in_folder);
    let report = trend::trend(folder, range, result_cache.as_ref(), plugin_host)?;
    write_report(&report.render(format)?, destination)?;
    Ok(Outcome::Passed)
}

fn run_impact(
    folder: &Path,
    request: &impact::ImpactRequest<'_>,
    destination: &ReportDestination,
    format: Option<ImpactFormat>,
) -> Result<Outcome, CliError> {
    let format = resolve_repo_format(
        folder,
        format,
        "impact",
        ImpactFormat::SUPPORTED,
        ImpactFormat::try_from_output_format,
    )?;
    let report = impact::impact(folder, request)?;
    write_report(&report.render(format)?, destination)?;
    Ok(Outcome::Passed)
}

fn run_trace(
    folder: &Path,
    from: &str,
    to: &str,
    destination: &ReportDestination,
    format: Option<TraceFormat>,
    options: &TraceOptions,
) -> Result<Outcome, CliError> {
    let format = resolve_repo_format(
        folder,
        format,
        "trace",
        TraceFormat::SUPPORTED,
        TraceFormat::try_from_output_format,
    )?;
    let report = trace::trace(
        folder,
        from,
        to,
        options.all,
        options.max_paths,
        options.package.as_deref(),
        &options.plugin_host,
    )?;
    write_report(&report.render(format)?, destination)?;
    Ok(Outcome::Passed)
}

fn write_report(report: &str, destination: &ReportDestination) -> Result<(), CliError> {
    let text = format!("{report}\n");
    match destination {
        ReportDestination::StandardOutput => print_report(&text),
        ReportDestination::File(path) => save_report(&text, path),
    }
}

fn print_report(text: &str) -> Result<(), CliError> {
    let mut output = std::io::stdout().lock();
    output
        .write_all(text.as_bytes())
        .and_then(|()| output.flush())
        .map_err(CliError::Output)
}

fn save_report(text: &str, path: &Path) -> Result<(), CliError> {
    std::fs::write(path, text).map_err(|source| CliError::OutputFile {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn describe(error: &dyn Error) -> String {
    let mut message = error.to_string();
    let mut cause = error.source();
    while let Some(source) = cause {
        message.push_str(": ");
        message.push_str(&source.to_string());
        cause = source.source();
    }
    message
}
