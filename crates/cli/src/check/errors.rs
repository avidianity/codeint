use std::fmt;

use codeint_config::{
    CodeownersError, ConfigFileKind, ConfigurationFileError, RatchetFileError,
    RatchetVersionMismatch, SqlDialectConflict,
};
use codeint_git::GitError;
use codeint_metrics::{CoverageApplyError, CoverageParseError};
use codeint_model::{
    InvalidReferenceRange, LanguagePluginError, MetricPluginError, RepoPath, ResolveError,
    WorkspaceError,
};
use codeint_plugin_host::PluginHostError;

#[derive(Debug)]
pub(crate) struct ConfigScopedError<E> {
    pub(crate) config_file: ConfigFileKind,
    pub(crate) error: E,
}

impl<E: fmt::Display> fmt::Display for ConfigScopedError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ConfigScopedError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.error.source()
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum CheckError {
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Configuration(#[from] ConfigurationFileError),
    #[error(transparent)]
    Codeowners(#[from] CodeownersError),
    #[error("Language plugin {plugin:?} cannot resolve runtime imports")]
    Resolve {
        plugin: String,
        #[source]
        source: ResolveError,
    },
    #[error("a Language plugin supplies an invalid Reference range")]
    ReferenceRange(#[source] InvalidReferenceRange),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    PluginHost(ConfigScopedError<PluginHostError>),
    #[error(transparent)]
    LanguagePlugin(ConfigScopedError<LanguagePluginError>),
    #[error(transparent)]
    MetricPlugin(ConfigScopedError<MetricPluginError>),
    #[error(
        "{}",
        format_extension_conflict(first, second, extension, path.as_ref(), *config_file)
    )]
    ExtensionConflict {
        config_file: ConfigFileKind,
        first: String,
        second: String,
        extension: String,
        path: Option<RepoPath>,
    },
    #[error("{}", format_unknown_workspace_package(requested, available))]
    UnknownWorkspacePackage {
        requested: String,
        available: Vec<String>,
    },
    #[error("Coverage report {path:?} does not exist")]
    MissingCoverageReport { path: String },
    #[error("Coverage report {path:?} is outside the Git repository")]
    CoverageReportOutsideRepository { path: String },
    #[error("cannot read Coverage report {path:?}")]
    UnreadableCoverageReport {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot parse Coverage report {path:?}")]
    InvalidCoverageReport {
        path: String,
        #[source]
        source: CoverageParseError,
    },
    #[error(transparent)]
    CoverageApply(#[from] CoverageApplyError),
    #[error(
        "budgets.production.test_gap ({budget}) is set, but Test gap is absent; the Test gap Budget needs a Coverage report"
    )]
    TestGapBudgetWithoutCoverage { budget: f64 },
    #[error("{config_file}: sql.dialects conflict: {conflict}")]
    SqlDialectConflict {
        config_file: ConfigFileKind,
        conflict: SqlDialectConflict,
    },
    #[error("{config_file} names an unknown Hazard line rule {rule:?} in hazards.disabled_rules")]
    UnknownHazardRule {
        config_file: ConfigFileKind,
        rule: String,
    },
    #[error(transparent)]
    RatchetFile(#[from] RatchetFileError),
    #[error(
        "Ratchet file {path:?} has stale Metric versions: {}; run `codeint ratchet update` to re-baseline",
        format_ratchet_mismatches(mismatches)
    )]
    RatchetVersionMismatch {
        path: String,
        mismatches: Vec<RatchetVersionMismatch>,
    },
}

fn format_ratchet_mismatches(mismatches: &[RatchetVersionMismatch]) -> String {
    mismatches
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

impl CheckError {
    pub(crate) fn plugin_host(config_file: ConfigFileKind, error: PluginHostError) -> CheckError {
        CheckError::PluginHost(ConfigScopedError { config_file, error })
    }

    pub(crate) fn language_plugin(
        config_file: ConfigFileKind,
        error: LanguagePluginError,
    ) -> CheckError {
        CheckError::LanguagePlugin(ConfigScopedError { config_file, error })
    }

    pub(crate) fn metric_plugin(
        config_file: ConfigFileKind,
        error: MetricPluginError,
    ) -> CheckError {
        CheckError::MetricPlugin(ConfigScopedError { config_file, error })
    }
}

fn format_unknown_workspace_package(requested: &str, available: &[String]) -> String {
    if available.is_empty() {
        format!(
            "Workspace package {requested:?} was not found; no Workspace packages were discovered in this repository"
        )
    } else {
        format!(
            "Workspace package {requested:?} was not found; discovered Workspace packages: {}",
            available.join(", ")
        )
    }
}

fn format_extension_conflict(
    first: &str,
    second: &str,
    extension: &str,
    path: Option<&RepoPath>,
    config_file: ConfigFileKind,
) -> String {
    match path {
        Some(path) => format!(
            "Language plugins {first} and {second} both claim extension .{extension} for {path}; assign separate directories in the languages setting of {config_file}"
        ),
        None => format!(
            "Language plugins {first} and {second} both claim extension .{extension}; assign separate directories in the languages setting of {config_file}"
        ),
    }
}
