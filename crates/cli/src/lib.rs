#![forbid(unsafe_code)]

mod check;
mod commit_measurement;
mod diff;
mod init;
mod mcp;
mod remedy;
mod trace;
mod trend;
mod update;

use std::error::Error;
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use codeint_config::{
    Command, ConfigError, DEFAULT_MAX_TRACE_PATHS, ReportDestination, read_invocation,
};
use codeint_git::ResultCache;
use codeint_report::ReportError;

pub use codeint_config::{
    CalendarDay, FindingsShown, PluginHostConfig, ReportFormat, TraceFormat, TrendFormat,
    TrendRange,
};
pub use codeint_report::{DiffReport, Report, TraceReport, TrendReport};

const DEFAULT_WORST_FINDINGS: usize = 20;
const DEFAULT_TREND_COMMITS: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckOptions {
    pub findings: FindingsShown,
    pub plugin_host: PluginHostConfig,
}

impl Default for CheckOptions {
    fn default() -> CheckOptions {
        CheckOptions {
            findings: FindingsShown::WorstPerMetric(DEFAULT_WORST_FINDINGS),
            plugin_host: PluginHostConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffOptions {
    pub head_ref: Option<String>,
    pub findings: FindingsShown,
    pub result_cache: Option<PathBuf>,
    pub plugin_host: PluginHostConfig,
}

impl Default for DiffOptions {
    fn default() -> DiffOptions {
        DiffOptions {
            head_ref: None,
            findings: FindingsShown::WorstPerMetric(DEFAULT_WORST_FINDINGS),
            result_cache: None,
            plugin_host: PluginHostConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrendOptions {
    pub range: TrendRange,
    pub result_cache: Option<PathBuf>,
    pub plugin_host: PluginHostConfig,
}

impl Default for TrendOptions {
    fn default() -> TrendOptions {
        TrendOptions {
            range: TrendRange::Latest(DEFAULT_TREND_COMMITS),
            result_cache: None,
            plugin_host: PluginHostConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceOptions {
    pub all: bool,
    pub max_paths: usize,
    pub plugin_host: PluginHostConfig,
}

impl Default for TraceOptions {
    fn default() -> TraceOptions {
        TraceOptions {
            all: false,
            max_paths: DEFAULT_MAX_TRACE_PATHS,
            plugin_host: PluginHostConfig::default(),
        }
    }
}

pub struct CheckOutcome {
    pub passed: bool,
    pub failures: Vec<String>,
    pub report: Report,
}

pub struct DiffOutcome {
    pub passed: bool,
    pub failures: Vec<String>,
    pub report: DiffReport,
}

#[derive(Debug)]
pub struct CodeintError {
    message: String,
    remedy: String,
}

impl CodeintError {
    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn remedy(&self) -> &str {
        &self.remedy
    }

    fn from_cli_error(error: CliError) -> CodeintError {
        CodeintError {
            message: describe(&error),
            remedy: remedy::remedy(&error).to_owned(),
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
    let report = check::check(folder, options.findings, &options.plugin_host)
        .map_err(|err| CodeintError::from_cli_error(CliError::Check(err)))?;
    let failures: Vec<String> = report
        .over_budget()
        .iter()
        .map(ToString::to_string)
        .collect();
    let passed = failures.is_empty();
    Ok(CheckOutcome {
        passed,
        failures,
        report,
    })
}

pub fn diff(
    folder: &Path,
    base_ref: &str,
    options: &DiffOptions,
) -> Result<DiffOutcome, CodeintError> {
    let report = measure_diff(folder, base_ref, options)
        .map_err(|err| CodeintError::from_cli_error(CliError::Check(err)))?;
    let failures = report.failure_messages();
    let passed = failures.is_empty();
    Ok(DiffOutcome {
        passed,
        failures,
        report,
    })
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
        cache.as_ref(),
        &options.plugin_host,
    )
}

pub fn trend(folder: &Path, options: &TrendOptions) -> Result<TrendReport, CodeintError> {
    let cache = options.result_cache.clone().map(ResultCache::in_folder);
    trend::trend(folder, options.range, cache.as_ref(), &options.plugin_host)
        .map_err(|err| CodeintError::from_cli_error(CliError::Check(err)))
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
        &options.plugin_host,
    )
    .map_err(|err| CodeintError::from_cli_error(CliError::Trace(err)))
}

pub fn init(folder: &Path) -> Result<PathBuf, CodeintError> {
    init::init(folder).map_err(|err| CodeintError::from_cli_error(CliError::Init(err)))
}

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Check(#[from] check::CheckError),
    #[error(transparent)]
    Trace(#[from] trace::TraceCommandError),
    #[error(transparent)]
    Init(#[from] init::InitError),
    #[error(transparent)]
    Update(#[from] update::UpdateError),
    #[error(transparent)]
    Report(#[from] ReportError),
    #[error("cannot write the report to standard output")]
    Output(#[source] std::io::Error),
    #[error("cannot write the report to {}", path.display())]
    OutputFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

const BUDGET_OR_REGRESSION_FAILED: u8 = 1;
const OPERATIONAL_ERROR: u8 = 2;

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
    match read_invocation()?.command {
        Command::Check {
            folder,
            destination,
            findings,
            format,
            plugin_host,
        } => run_check(&folder, &destination, findings, format, &plugin_host),
        Command::Diff {
            folder,
            base_ref,
            head_ref,
            destination,
            findings,
            format,
            result_cache,
            plugin_host,
        } => {
            let options = DiffOptions {
                head_ref,
                findings,
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
        Command::Trace {
            folder,
            from,
            to,
            all,
            max_paths,
            destination,
            format,
            plugin_host,
        } => {
            let options = TraceOptions {
                all,
                max_paths,
                plugin_host,
            };
            run_trace(&folder, &from, &to, &destination, format, &options)
        }
        Command::Init { folder } => run_init(&folder),
        Command::Update {
            check_only,
            version,
            release_base_url,
        } => run_update(check_only, version.as_deref(), release_base_url.as_deref()),
        Command::Mcp {
            folder,
            result_cache,
            plugin_host,
        } => run_mcp(&folder, result_cache, plugin_host),
    }
}

fn run_init(folder: &Path) -> Result<Outcome, CliError> {
    let written = init::init(folder)?;
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

fn run_check(
    folder: &Path,
    destination: &ReportDestination,
    findings: FindingsShown,
    format: ReportFormat,
    plugin_host: &PluginHostConfig,
) -> Result<Outcome, CliError> {
    let report = check::check(folder, findings, plugin_host)?;
    write_report(&report.render(format)?, destination)?;
    Ok(Outcome::from_failures(
        report
            .over_budget()
            .iter()
            .map(ToString::to_string)
            .collect(),
    ))
}

fn run_diff(
    folder: &Path,
    base_ref: &str,
    destination: &ReportDestination,
    format: ReportFormat,
    options: &DiffOptions,
) -> Result<Outcome, CliError> {
    let report = measure_diff(folder, base_ref, options)?;
    write_report(&report.render(format)?, destination)?;
    Ok(Outcome::from_failures(report.failure_messages()))
}

fn run_trend(
    folder: &Path,
    range: TrendRange,
    destination: &ReportDestination,
    format: TrendFormat,
    result_cache: Option<PathBuf>,
    plugin_host: &PluginHostConfig,
) -> Result<Outcome, CliError> {
    let result_cache = result_cache.map(ResultCache::in_folder);
    let report = trend::trend(folder, range, result_cache.as_ref(), plugin_host)?;
    write_report(&report.render(format)?, destination)?;
    Ok(Outcome::Passed)
}

fn run_trace(
    folder: &Path,
    from: &str,
    to: &str,
    destination: &ReportDestination,
    format: TraceFormat,
    options: &TraceOptions,
) -> Result<Outcome, CliError> {
    let report = trace::trace(
        folder,
        from,
        to,
        options.all,
        options.max_paths,
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
