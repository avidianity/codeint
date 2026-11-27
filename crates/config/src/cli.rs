use std::path::{Path, PathBuf};

use clap::Parser;

use crate::PluginLimits;
use crate::agent_home;
use crate::calendar_day::CalendarDay;
use crate::churn::{ChurnSince, ChurnWindow};
use crate::command::{
    AgentCommand, BaselineCommand, Command, ConfigError, ConfigFormat,
    DEFAULT_PLUGIN_DOWNLOAD_TIMEOUT_SECONDS, DEFAULT_PLUGIN_MAX_DOWNLOAD_BYTES, FindingsShown,
    Invocation, PluginCommand, PluginHostConfig, RatchetCommand, ReportDestination, ReportInput,
    ReportRenderFormat, SchemaFormat, TrendFormat, TrendRange, WORST_FINDINGS_PER_METRIC,
};
use crate::coupling::{CouplingRatio, CouplingSettings};
use crate::resolved::ConfigOverrides;
use crate::result_cache_folder;

mod args;

pub(crate) use args::Arguments;

use args::{
    AgentSubcommandArguments, BaselineSubcommandArguments, CommandArguments,
    HistoryReportArguments, ImpactArguments, PluginSubcommandArguments, RatchetSubcommandArguments,
    ReportArguments, TraceArguments,
};

const TEST_CA_CERT_VARIABLE: &str = "CODEINT_TEST_CA_CERT";
const RELEASE_BASE_URL_VARIABLE: &str = "CODEINT_RELEASE_BASE_URL";
const PLUGIN_CACHE_SUBDIR: &str = "plugins";

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
        CommandArguments::Check {
            path,
            coverage_reports,
            ignore_baseline,
            report,
        } => build_check_command(
            current_folder,
            path,
            CheckSelection {
                coverage_reports,
                ignore_baseline,
            },
            report,
        ),
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
        CommandArguments::Init { adopt, format } => Ok(Command::Init {
            folder: current_folder.to_path_buf(),
            adopt,
            format,
            plugin_host: plugin_host_config_of_this_process(current_folder),
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
        other => build_tooling_command(current_folder, other),
    }
}

fn build_tooling_command(
    current_folder: &Path,
    arguments: CommandArguments,
) -> Result<Command, ConfigError> {
    match arguments {
        CommandArguments::Schema { output, format } => {
            build_schema_command(current_folder, output.output, format)
        }
        CommandArguments::Config {
            path,
            resolved: _,
            format,
            budgets,
            source_sets,
            plugin_limits,
        } => build_config_command(
            current_folder,
            path,
            format,
            ConfigOverrides {
                budgets,
                source_sets,
                plugin_memory_mb: plugin_limits.memory_mb,
                plugin_timeout_ms: plugin_limits.timeout_ms,
            },
        ),
        CommandArguments::Report {
            input,
            output,
            all,
            format,
        } => build_report_command(current_folder, input, output.output, all, format),
        CommandArguments::Doctor { path, format } => Ok(Command::Doctor {
            folder: existing_folder(current_folder, path)?,
            format,
            plugin_host: plugin_host_config_of_this_process(current_folder),
        }),
        CommandArguments::Agent { command } => build_agent_command(current_folder, command),
        CommandArguments::Ratchet { command } => build_ratchet_command(current_folder, command),
        CommandArguments::Plugin { command } => Ok(build_plugin_command(current_folder, command)),
        CommandArguments::Baseline { command } => build_baseline_command(current_folder, command),
        CommandArguments::Check { .. }
        | CommandArguments::Diff { .. }
        | CommandArguments::Trend { .. }
        | CommandArguments::Impact(_)
        | CommandArguments::Trace(_)
        | CommandArguments::Init { .. }
        | CommandArguments::Update { .. }
        | CommandArguments::Mcp
        | CommandArguments::Guard { .. }
        | CommandArguments::Churn { .. }
        | CommandArguments::Coupling { .. } => unreachable!("handled by build_command"),
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
    all: bool,
    format: Option<ReportRenderFormat>,
) -> Result<Command, ConfigError> {
    let source = if input.as_os_str() == "-" {
        ReportInput::StandardInput
    } else {
        ReportInput::File(input)
    };
    Ok(Command::Report {
        folder: current_folder.to_path_buf(),
        input: source,
        destination: report_destination(current_folder, output)?,
        all,
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

fn build_agent_command(
    current_folder: &Path,
    arguments: AgentSubcommandArguments,
) -> Result<Command, ConfigError> {
    match arguments {
        AgentSubcommandArguments::Status {
            client,
            format,
            config_root,
        } => Ok(Command::Agent(AgentCommand::Status {
            config_root: agent_home_of_this_process(current_folder, config_root.as_deref())?,
            client,
            format,
        })),
        AgentSubcommandArguments::Install {
            dry_run,
            client,
            format,
            config_root,
        } => Ok(Command::Agent(AgentCommand::Install {
            config_root: agent_home_of_this_process(current_folder, config_root.as_deref())?,
            client,
            dry_run,
            format,
        })),
    }
}

fn build_plugin_command(current_folder: &Path, arguments: PluginSubcommandArguments) -> Command {
    match arguments {
        PluginSubcommandArguments::Test {
            path,
            kind,
            sources,
            plugin_limits,
            format,
        } => {
            let defaults = PluginLimits::default();
            Command::Plugin(PluginCommand::Test {
                path: current_folder.join(path),
                kind,
                sources: sources
                    .into_iter()
                    .map(|source| current_folder.join(source))
                    .collect(),
                limits: PluginLimits {
                    memory_mb: plugin_limits.memory_mb.unwrap_or(defaults.memory_mb),
                    timeout_ms: plugin_limits.timeout_ms.unwrap_or(defaults.timeout_ms),
                },
                format,
            })
        }
    }
}

fn build_ratchet_command(
    current_folder: &Path,
    arguments: RatchetSubcommandArguments,
) -> Result<Command, ConfigError> {
    match arguments {
        RatchetSubcommandArguments::Update {
            path,
            reset,
            coverage_reports,
            format,
        } => Ok(Command::Ratchet(RatchetCommand::Update {
            folder: existing_folder(current_folder, path)?,
            reset,
            coverage_reports,
            format,
            plugin_host: plugin_host_config_of_this_process(current_folder),
        })),
    }
}

fn build_baseline_command(
    current_folder: &Path,
    arguments: BaselineSubcommandArguments,
) -> Result<Command, ConfigError> {
    match arguments {
        BaselineSubcommandArguments::Update {
            path,
            reset,
            coverage_reports,
            format,
        } => Ok(Command::Baseline(BaselineCommand::Update {
            folder: existing_folder(current_folder, path)?,
            reset,
            coverage_reports,
            format,
            plugin_host: plugin_host_config_of_this_process(current_folder),
        })),
    }
}

fn agent_home_of_this_process(
    current_folder: &Path,
    explicit: Option<&Path>,
) -> Result<PathBuf, ConfigError> {
    agent_home::resolve_agent_home(current_folder, explicit, cfg!(windows), std::env::var_os)
        .ok_or(ConfigError::MissingAgentHome)
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
        destination: report_destination(current_folder, history.output.output)?,
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
        destination: report_destination(current_folder, history.output.output)?,
        findings: findings_shown(history.all),
        format: history.format,
        plugin_host: plugin_host_config_of_this_process(current_folder),
    })
}

struct CheckSelection {
    coverage_reports: Vec<PathBuf>,
    ignore_baseline: bool,
}

fn build_check_command(
    current_folder: &Path,
    path: Option<PathBuf>,
    selection: CheckSelection,
    report: ReportArguments,
) -> Result<Command, ConfigError> {
    Ok(Command::Check {
        folder: existing_folder(current_folder, path)?,
        destination: report_destination(current_folder, report.output.output)?,
        findings: findings_shown(report.all),
        format: report.format,
        package: report.package,
        owner: report.owner,
        coverage_reports: selection.coverage_reports,
        ignore_baseline: selection.ignore_baseline,
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
    format: Option<TrendFormat>,
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

fn read_env_var(name: &str) -> Option<std::ffi::OsString> {
    std::env::var_os(name)
}

fn result_cache_folder_of_this_process(current_folder: &Path) -> Option<PathBuf> {
    result_cache_folder::result_cache_folder(
        current_folder,
        result_cache_folder::CacheFolderConvention::of_this_platform(),
        read_env_var,
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
        max_download_bytes: DEFAULT_PLUGIN_MAX_DOWNLOAD_BYTES,
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
