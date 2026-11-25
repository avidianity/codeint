use std::borrow::Cow;

use codeint_config::{
    CodeownersError, ConfigError, ConfigFileKind, ConfigurationFileError, RepoStateFile,
    StarterConfigurationError, StateFileError,
};
use codeint_git::GitError;
use codeint_metrics::ImpactError;
use codeint_model::{LanguagePluginError, MetricPluginError, WorkspaceError};
use codeint_plugin_host::PluginHostError;
use codeint_report::{SavedReportError, TraceError};

use crate::CliError;
use crate::agent::AgentError;
use crate::check::CheckError;
use crate::config::ConfigCommandError;
use crate::guard::GuardError;
use crate::impact::ImpactCommandError;
use crate::init::InitError;
use crate::ratchet::RatchetError;
use crate::report::RerenderError;
use crate::trace::TraceCommandError;
use crate::update::UpdateError;

pub(crate) fn remedy(error: &CliError) -> Cow<'static, str> {
    use CliError::*;
    match error {
        Config(error) => Cow::Borrowed(config_remedy(error)),
        ConfigCommand(error) => config_command_remedy(error),
        Check(error) => check_remedy(error),
        Guard(error) => guard_remedy(error),
        Impact(error) => impact_remedy(error),
        Trace(error) => trace_remedy(error),
        other => tooling_remedy(other),
    }
}

pub(crate) fn mcp_remedy(error: &CliError) -> Cow<'static, str> {
    match error {
        CliError::Check(CheckError::BaselineVersionMismatch { .. }) => Cow::Borrowed(
            "Run codeint baseline update to refresh the Baseline for the updated Metric versions, or pass ignore_baseline: true to show every Finding.",
        ),
        other => remedy(other),
    }
}

fn tooling_remedy(error: &CliError) -> Cow<'static, str> {
    use CliError::*;
    match error {
        Agent(error) => Cow::Borrowed(agent_remedy(error)),
        Ratchet(error) => ratchet_remedy(error),
        Init(error) => init_remedy(error),
        Update(error) => Cow::Borrowed(update_remedy(error)),
        Report(_) => Cow::Borrowed(
            "This is a defect in codeint; report it at https://github.com/avidianity/codeint/issues.",
        ),
        Rerender(error) => Cow::Borrowed(rerender_remedy(error)),
        Output(_) => Cow::Borrowed("Keep standard output open until codeint exits."),
        OutputFile { .. } => Cow::Borrowed(
            "Pass --output a file path that you have permission to write, not a folder.",
        ),
        Config(_) | ConfigCommand(_) | Check(_) | Guard(_) | Impact(_) | Trace(_) => {
            unreachable!("handled by remedy")
        }
    }
}

fn ratchet_remedy(error: &RatchetError) -> Cow<'static, str> {
    match error {
        RatchetError::RatchetFile(error) => state_file_remedy(error),
        RatchetError::LooseningRefused { .. } => Cow::Borrowed(
            "Fix the regressed Metrics, or run codeint ratchet update --reset to intentionally loosen unchanged-version Ratchet ceilings.",
        ),
    }
}

fn state_file_remedy(error: &StateFileError) -> Cow<'static, str> {
    let file = error.state_file();
    let command = match file {
        RepoStateFile::Ratchet => "codeint ratchet update",
        RepoStateFile::Baseline => "codeint baseline update",
    };
    match error {
        StateFileError::OutsideRepository { .. } => Cow::Borrowed(match file {
            RepoStateFile::Ratchet => {
                "Configure ratchet.path to a repository-relative path without \"..\" or symlinks escaping the repository."
            }
            RepoStateFile::Baseline => {
                "Make .codeint/baseline.json a regular file inside the repository, not a symbolic link that leaves it."
            }
        }),
        StateFileError::BrokenSymlink { .. } => Cow::Owned(format!(
            "Point the {file} symlink to an existing file inside the repository, or remove it and run {command}."
        )),
        StateFileError::Unreadable { .. } => Cow::Owned(format!(
            "Check that the {file} file is readable, then run codeint again."
        )),
        StateFileError::InvalidJson { .. }
        | StateFileError::UnsupportedSchemaVersion { .. }
        | StateFileError::InvalidCeiling { .. }
        | StateFileError::InvalidMetricVersion { .. }
        | StateFileError::UnknownDependencyMetric { .. } => Cow::Owned(format!(
            "Fix the {file} file syntax and values, or regenerate it with {command} --reset."
        )),
        StateFileError::Write { .. } => Cow::Owned(format!(
            "Check that you have permission to create and write the {file} file inside the repository, then retry."
        )),
    }
}

fn trace_remedy(error: &TraceCommandError) -> Cow<'static, str> {
    match error {
        TraceCommandError::Check(error) => check_remedy(error),
        TraceCommandError::UnknownModule { .. }
        | TraceCommandError::Trace(TraceError::UnknownModule { .. }) => Cow::Borrowed(
            "Pass repository-relative paths of files measured by codeint (not ignored, unparsable, declaration-only, or outside the repository).",
        ),
        TraceCommandError::Trace(TraceError::InvalidMaxPaths) => {
            Cow::Borrowed("Pass a positive integer (at least 1) for --max-paths.")
        }
    }
}

fn check_remedy(error: &CheckError) -> Cow<'static, str> {
    if let Some(remedy) = repository_state_or_coverage_remedy(error) {
        return remedy;
    }
    match error {
        CheckError::ExtensionConflict { config_file, .. } => Cow::Owned(format!(
            "Assign separate directories to each Language plugin in the languages setting of {config_file}."
        )),
        CheckError::UnknownWorkspacePackage { .. } => Cow::Borrowed(
            "Pass one of the discovered Workspace package paths to --package, or omit --package to measure the whole repository.",
        ),
        CheckError::SqlDialectConflict { config_file, .. } => Cow::Owned(format!(
            "Adjust the include globs in sql.dialects in {config_file} so each SQL file matches at most one Dialect."
        )),
        CheckError::UnknownHazardRule { config_file, .. } => Cow::Owned(format!(
            "Remove the unknown rule name from hazards.disabled_rules in {config_file}, or run codeint schema to list the known Hazard line rules."
        )),
        CheckError::Git(error) => Cow::Borrowed(git_remedy(error)),
        CheckError::Configuration(error) => configuration_file_remedy(error),
        CheckError::Codeowners(error) => Cow::Borrowed(codeowners_remedy(error)),
        CheckError::Workspace(error) => Cow::Borrowed(workspace_remedy(error)),
        other => check_plugin_remedy(other),
    }
}

fn repository_state_or_coverage_remedy(error: &CheckError) -> Option<Cow<'static, str>> {
    match error {
        CheckError::StateFile(error) => Some(state_file_remedy(error)),
        CheckError::RatchetVersionMismatch { .. } => Some(Cow::Borrowed(
            "Run codeint ratchet update to re-baseline the Ratchet file for the updated Metric versions.",
        )),
        CheckError::BaselineVersionMismatch { .. } => Some(Cow::Borrowed(
            "Run codeint baseline update to refresh the Baseline for the updated Metric versions, or pass --ignore-baseline to show every Finding.",
        )),
        other => coverage_check_remedy(other).map(Cow::Borrowed),
    }
}

fn coverage_check_remedy(error: &CheckError) -> Option<&'static str> {
    Some(match error {
        CheckError::MissingCoverageReport { .. } => {
            "Check that the Coverage report path exists and generate it with your test runner before running codeint check."
        }
        CheckError::CoverageReportOutsideRepository { .. } => {
            "Pass a Coverage report file inside the Git repository; codeint refuses paths and symbolic links outside the repository."
        }
        CheckError::UnreadableCoverageReport { .. } => {
            "Check that you have permission to read the Coverage report file, then run codeint check again."
        }
        CheckError::InvalidCoverageReport { .. } => {
            "Pass a valid LCOV or Cobertura XML Coverage report within the size and XML nesting limits."
        }
        CheckError::CoverageApply(_) => {
            "Regenerate the Coverage report from the current working tree so its line numbers match the repository files."
        }
        CheckError::TestGapBudgetWithoutCoverage { .. } => {
            "Name at least one LCOV or Cobertura XML Coverage report in coverage_reports or pass --coverage-report <PATH>, or remove budgets.production.test_gap."
        }
        _ => return None,
    })
}

fn check_plugin_remedy(error: &CheckError) -> Cow<'static, str> {
    match error {
        CheckError::Resolve { .. } => Cow::Borrowed(
            "This is a defect in the Language plugin; report it to the author of the Language plugin.",
        ),
        CheckError::ReferenceRange(_) => Cow::Borrowed(
            "Fix the Reference ranges that the Language plugin supplies, or report the defect to its author.",
        ),
        CheckError::PluginHost(scoped) => plugin_host_remedy(&scoped.error, scoped.config_file),
        CheckError::LanguagePlugin(scoped) => {
            language_plugin_remedy(&scoped.error, scoped.config_file)
        }
        CheckError::MetricPlugin(scoped) => metric_plugin_remedy(&scoped.error, scoped.config_file),
        CheckError::ExtensionConflict { .. }
        | CheckError::UnknownWorkspacePackage { .. }
        | CheckError::MissingCoverageReport { .. }
        | CheckError::CoverageReportOutsideRepository { .. }
        | CheckError::UnreadableCoverageReport { .. }
        | CheckError::InvalidCoverageReport { .. }
        | CheckError::CoverageApply(_)
        | CheckError::TestGapBudgetWithoutCoverage { .. }
        | CheckError::SqlDialectConflict { .. }
        | CheckError::UnknownHazardRule { .. }
        | CheckError::StateFile(_)
        | CheckError::RatchetVersionMismatch { .. }
        | CheckError::BaselineVersionMismatch { .. }
        | CheckError::Git(_)
        | CheckError::Configuration(_)
        | CheckError::Codeowners(_)
        | CheckError::Workspace(_) => unreachable!("handled by check_remedy"),
    }
}

fn config_command_remedy(error: &ConfigCommandError) -> Cow<'static, str> {
    match error {
        ConfigCommandError::Git(error) => Cow::Borrowed(git_remedy(error)),
        ConfigCommandError::Configuration(error) => configuration_file_remedy(error),
    }
}

fn guard_remedy(error: &GuardError) -> Cow<'static, str> {
    match error {
        GuardError::Git(error) => Cow::Borrowed(git_remedy(error)),
        GuardError::Configuration(error) => configuration_file_remedy(error),
        GuardError::OutsideRepository { .. } => Cow::Borrowed(
            "Pass a file path inside the Git repository, such as src/domain/service.ts.",
        ),
        GuardError::NonUtf8Path { .. } | GuardError::InvalidPath { .. } => {
            Cow::Borrowed("Pass a valid UTF-8 file path inside the Git repository.")
        }
    }
}

fn agent_remedy(error: &AgentError) -> &'static str {
    use AgentError as E;
    match error {
        E::Read { .. } | E::CreateDir { .. } | E::Write { .. } => {
            "Check file and folder permissions for the agent configuration path, then retry."
        }
        E::DanglingSymlink { .. } => {
            "Point the symlink to an existing configuration file, or remove the dangling symlink and retry."
        }
        E::InvalidJson { .. }
        | E::ExpectedRootObject { .. }
        | E::InvalidToml { .. }
        | E::InvalidMcpSection { .. }
        | E::InvalidCodeintEntry { .. } => {
            "Fix the agent configuration file syntax and ensure its MCP servers section maps names to objects."
        }
        E::SerializeJson { .. } | E::SerializeToml { .. } => {
            "This is a defect in codeint; report it at https://github.com/avidianity/codeint/issues."
        }
    }
}

fn impact_remedy(error: &ImpactCommandError) -> Cow<'static, str> {
    match error {
        ImpactCommandError::Check(error) => check_remedy(error),
        ImpactCommandError::Analysis(ImpactError::TargetNotInCodeModel { .. }) => Cow::Borrowed(
            "Pass a tracked or non-ignored source file that a Language plugin measures, or check the file path.",
        ),
        ImpactCommandError::MissingTargets => Cow::Borrowed(
            "Pass one or more repository file paths, or pass --changed <base> to analyze changed files.",
        ),
        ImpactCommandError::PathOutsideRepository { .. } => {
            Cow::Borrowed("Pass a file path inside the Git repository.")
        }
        ImpactCommandError::InvalidTargetPath { .. } => {
            Cow::Borrowed("Pass a repository-relative file path using forward slashes.")
        }
    }
}

fn init_remedy(error: &InitError) -> Cow<'static, str> {
    match error {
        InitError::Git(GitError::NotInRepository { .. }) => Cow::Borrowed(
            "Run codeint init inside a Git repository; it writes codeint.jsonc at the root of the repository.",
        ),
        InitError::Git(error) => Cow::Borrowed(git_remedy(error)),
        InitError::StarterConfiguration(error) => starter_configuration_remedy(error),
        InitError::AdoptConfigurationExists(_) => Cow::Borrowed(
            "This repository already has a codeint config; run codeint ratchet update and codeint baseline update to create the Ratchet and Baseline for it.",
        ),
        InitError::Check(error) => check_remedy(error),
        InitError::Ratchet(error) => ratchet_remedy(error),
        InitError::StateFileExists { update_command, .. } => Cow::Owned(format!(
            "Keep it and run {update_command} to tighten it, run {update_command} --reset to replace an invalid file, or delete it and run codeint init --adopt again."
        )),
        InitError::RollbackIncomplete { .. } => Cow::Borrowed(
            "Delete the listed files, fix the first error, and run codeint init --adopt again.",
        ),
        InitError::Inspect { .. } => Cow::Borrowed(
            "Check the permissions of the .codeint folder and run codeint init --adopt again.",
        ),
    }
}

fn update_remedy(error: &UpdateError) -> &'static str {
    match error {
        UpdateError::ManagedByPackageManager { .. } => {
            "Update codeint through your package manager instead, such as npm update -g codeint or npx codeint@latest."
        }
        UpdateError::ChecksumMismatch { .. } | UpdateError::MissingChecksum { .. } => {
            "Do not use the downloaded archive; retry codeint update or verify the release on GitHub."
        }
        UpdateError::LatestRelease { .. } | UpdateError::Download { .. } => {
            "Check your network connection and GitHub Releases availability, then run codeint update again."
        }
        UpdateError::Replace { .. } => {
            "Ensure you have write permission to the folder containing codeint and retry."
        }
        UpdateError::CurrentExecutable(_)
        | UpdateError::UnsupportedPlatform
        | UpdateError::ChecksumTool { .. }
        | UpdateError::Extract { .. } => {
            "Reinstall codeint from https://github.com/avidianity/codeint/releases."
        }
    }
}

fn config_remedy(error: &ConfigError) -> &'static str {
    match error {
        ConfigError::CurrentFolder(_) => {
            "Run codeint from a folder that exists and that you have permission to open."
        }
        ConfigError::MissingPath { .. } => {
            "Pass a folder that exists, or leave the path out to measure the current folder."
        }
        ConfigError::NotAFolder { .. } => {
            "Pass a folder, not a file; codeint measures the whole working tree that contains it."
        }
        ConfigError::UnreadablePath { .. } => "Pass a folder that you have permission to open.",
        ConfigError::MissingOutputFolder { .. } => {
            "Create the folder first, or pass --output a file path in a folder that exists."
        }
        ConfigError::MissingAgentHome => {
            "Pass --config-root <DIR>, or set CODEINT_AGENT_HOME or HOME."
        }
    }
}

pub(crate) fn git_remedy(error: &GitError) -> &'static str {
    match error {
        GitError::NotInRepository { .. } => {
            "Run codeint inside a Git repository, or pass a folder inside one: codeint check <path>"
        }
        GitError::BareRepository { .. } => {
            "Run codeint in a clone that has a working tree, not in a bare repository."
        }
        GitError::Open { .. } | GitError::Walk(_) => {
            "Check that git status works in this repository, then run codeint again."
        }
        GitError::NonUtf8Path { .. } | GitError::InvalidPath(_) => {
            "Rename the file so that its path is valid UTF-8 with no backslash, then run codeint again."
        }
        GitError::Read { .. } => {
            "Check that you have permission to read the file, then run codeint again."
        }
        GitError::UnknownRef { .. } => {
            "Pass a branch, tag, or commit that exists in this repository; fetch it first when it lives on a remote: git fetch origin main"
        }
        GitError::NoHeadCommit { .. } => {
            "Commit once, then compare again; codeint diff measures the merge-base of the base ref and HEAD."
        }
        GitError::NoMergeBase { .. } => {
            "Pass a base ref that shares history with the head, which is HEAD unless you pass --head; in a shallow clone, fetch more history: git fetch --unshallow"
        }
        GitError::NoHistory(_) => {
            "Commit once, then run codeint again; history commands measure the commits of the current branch."
        }
        GitError::MergeBase { .. }
        | GitError::History { .. }
        | GitError::CommitTree { .. }
        | GitError::ReadBlob { .. } => {
            "Check that git log and git show work in this repository, then run codeint again."
        }
    }
}

fn workspace_remedy(error: &WorkspaceError) -> &'static str {
    match error {
        WorkspaceError::Manifest { .. } => {
            "Fix the manifest that the message names so that your package manager accepts it, then run codeint again."
        }
        WorkspaceError::PluginFailed { .. } | WorkspaceError::InvalidPackageRoot(_) => {
            "This is a defect in the Language plugin; report it to the Plugin's author."
        }
    }
}

pub(crate) fn configuration_file_remedy(error: &ConfigurationFileError) -> Cow<'static, str> {
    plugin_config_remedy(error)
        .or_else(|| test_gap_config_remedy(error))
        .or_else(|| sql_hazards_and_ratchet_config_remedy(error))
        .or_else(|| boundary_config_remedy(error))
        .or_else(|| file_access_remedy(error))
        .or_else(|| json_shape_remedy(error).map(Cow::Borrowed))
        .unwrap_or(Cow::Borrowed(
            "Check the configuration file against the JSON schema that $schema links to.",
        ))
}

fn file_access_remedy(error: &ConfigurationFileError) -> Option<Cow<'static, str>> {
    use ConfigurationFileError as E;
    match error {
        E::AmbiguousConfigFiles { .. } => Some(Cow::Borrowed(
            "Keep only one configuration file among codeint.json, codeint.jsonc, .codeint/config.json, and .codeint/config.jsonc, and remove the others.",
        )),
        E::Unreadable { file, .. } => Some(Cow::Owned(format!(
            "Check that {file} is a file you have permission to read, then run codeint again."
        ))),
        E::BrokenSymlink { file } => Some(Cow::Owned(format!(
            "Point the {file} symlink to an existing file, or remove the broken symlink and run codeint again."
        ))),
        E::NotJson { file, .. } | E::InvalidJsonc { file, .. } => Some(Cow::Owned(format!(
            "Fix the JSON syntax of {file} at that line and column."
        ))),
        E::NewerSchemaVersion { file, .. } => Some(Cow::Owned(format!(
            "Run codeint update (or upgrade the codeint npm package) to match or exceed the $schema version, or lower the $schema tag in {file}."
        ))),
        _ => None,
    }
}

fn json_shape_remedy(error: &ConfigurationFileError) -> Option<&'static str> {
    use ConfigurationFileError::*;
    Some(match error {
        UnknownKey { .. } => "Remove the key, or correct its spelling to one of the allowed keys.",
        UnknownCliBudgetMetric { .. } => {
            "Pass one of the allowed Metric names to --budget, such as production.erosion=0.25."
        }
        WrongType { .. } => {
            "Change the value to the expected type; the JSON schema that $schema links to describes every key."
        }
        OutOfRange {
            minimum, maximum, ..
        } => out_of_range_remedy(*minimum, *maximum),
        OutOfExclusiveMinRange {
            exclusive_minimum,
            maximum,
            ..
        } => out_of_exclusive_min_range_remedy(*exclusive_minimum, *maximum),
        NotOneOf { .. } => {
            "Set the value to one of the allowed values; the JSON schema that $schema links to describes each one."
        }
        MissingKey { .. } => {
            "Add the key; the JSON schema that $schema links to describes every key."
        }
        _ => return None,
    })
}

fn out_of_range_remedy(minimum: f64, maximum: f64) -> &'static str {
    if minimum == 0.0 && maximum == 1.0 {
        "Set the value to a ratio from 0 to 1, as the report shows ratios."
    } else {
        "Set the value within the allowed range; the JSON schema that $schema links to describes every key."
    }
}

fn out_of_exclusive_min_range_remedy(exclusive_minimum: f64, maximum: f64) -> &'static str {
    if exclusive_minimum == 0.0 && maximum == 1.0 {
        "Set the value to a ratio greater than 0 and at most 1, such as 0.85."
    } else {
        "Set the value within the allowed range; the JSON schema that $schema links to describes every key."
    }
}

fn boundary_config_remedy(error: &ConfigurationFileError) -> Option<Cow<'static, str>> {
    use ConfigurationFileError::*;
    match error {
        InvalidBoundaryZoneName { file, .. } => Some(Cow::Owned(format!(
            "Give each boundary zone in {file} a non-empty name other than \"*\"."
        ))),
        EmptyBoundaryZonePatterns { .. } => Some(Cow::Borrowed(
            "Add at least one path glob under patterns for every zone in boundaries.zones.",
        )),
        UnknownBoundaryZone { .. } => Some(Cow::Borrowed(
            "Reference only zones defined under boundaries.zones (or \"*\") in boundaries.rules.",
        )),
        ConflictingBoundaryRule { .. } => Some(Cow::Borrowed(
            "Set either allow or deny on each rule in boundaries.rules, not both.",
        )),
        ConflictingExternalBoundaryRule { .. } => Some(Cow::Borrowed(
            "Set either allow_external or deny_external on each rule in boundaries.rules, not both.",
        )),
        MissingBoundaryRuleDirective { .. } => Some(Cow::Borrowed(
            "Set at least one of allow, deny, allow_external, or deny_external on each rule in boundaries.rules.",
        )),
        ConflictingBoundaryDirectives { .. } => Some(Cow::Borrowed(
            "Use either allow rules or deny rules for a given source zone in boundaries.rules, not both.",
        )),
        ConflictingExternalBoundaryDirectives { .. } => Some(Cow::Borrowed(
            "Use either allow_external rules or deny_external rules for a given source zone in boundaries.rules, not both.",
        )),
        EmptyBoundaryExternalPattern { file, .. } => Some(Cow::Owned(format!(
            "Set non-empty package globs in allow_external or deny_external in {file}, or remove the empty entry."
        ))),
        _ => None,
    }
}

fn plugin_config_remedy(error: &ConfigurationFileError) -> Option<Cow<'static, str>> {
    use ConfigurationFileError::*;
    Some(match error {
        UnknownLanguagePlugin { .. } => Cow::Borrowed(
            "Name one of the Language plugins listed, or remove the entry from languages.",
        ),
        InvalidGlob { .. } | InvalidCliGlob { .. } => Cow::Borrowed(
            "Fix the glob: * matches within one folder, ** matches across folders, and {a,b} matches either a or b.",
        ),
        InvalidSha256 { .. } => Cow::Borrowed(
            "Set sha256 to the 64-character hexadecimal SHA-256 digest of the plugin file.",
        ),
        EmptyPluginField { file, .. } => Cow::Owned(format!(
            "Set a non-empty value for the plugin field in {file}."
        )),
        MissingPluginSource { file, .. } => Cow::Owned(format!(
            "Set either path (for a local file) or url (for an https:// download) on the plugin entry in {file}."
        )),
        AmbiguousPluginSource { file, .. } => Cow::Owned(format!(
            "Keep either path or url on the plugin entry in {file} and remove the other."
        )),
        NonHttpsPluginUrl { file, .. } => Cow::Owned(format!(
            "Use an https:// URL for the plugin in {file}; plain HTTP and other schemes are refused."
        )),
        EmptyDependencyIgnore { file, .. } => Cow::Owned(format!(
            "Set non-empty package names in dependencies.ignore in {file}, or remove the empty entry."
        )),
        _ => return None,
    })
}

fn test_gap_config_remedy(error: &ConfigurationFileError) -> Option<Cow<'static, str>> {
    use ConfigurationFileError::*;
    Some(match error {
        EmptyCoverageReportPath { file, .. } => Cow::Owned(format!(
            "Set non-empty paths in coverage_reports in {file}, or remove the empty entry."
        )),
        AbsoluteCoverageReportPath { file, .. }
        | ParentTraversalCoverageReportPath { file, .. } => Cow::Owned(format!(
            "Use repository-relative paths without \"..\" in coverage_reports in {file}."
        )),
        TestGapBudgetOutsideProduction { file, key } => {
            test_gap_outside_production_remedy(*file, key.first_key())
        }
        CliTestGapBudgetOutsideProduction { .. } => Cow::Borrowed(
            "Pass --budget production.test_gap=RATIO; Test gap covers only the Production Source set.",
        ),
        TestGapRegressionLimit { file, .. } => Cow::Owned(format!(
            "Remove test_gap from regression_limits in {file}; Test gap is checked with budgets.production.test_gap only."
        )),
        MetricBudgetOutsideProduction { file, key, .. } => {
            metric_budget_outside_production_remedy(*file, key.first_key())
        }
        CliMetricBudgetOutsideProduction { .. } => Cow::Borrowed(
            "Pass --budget production.<metric>=RATIO; this Metric applies to the Production Source set only.",
        ),
        MetricRegressionLimitOutsideProduction { file, .. } => Cow::Owned(format!(
            "Move this Regression limit to regression_limits.production or top-level regression_limits in {file}, or remove it."
        )),
        _ => return None,
    })
}

fn test_gap_outside_production_remedy(
    file: ConfigFileKind,
    section: Option<&str>,
) -> Cow<'static, str> {
    if section == Some("new_file_budgets") {
        Cow::Owned(format!(
            "Move the test_gap Budget to new_file_budgets.production.test_gap in {file}, or remove it."
        ))
    } else {
        Cow::Owned(format!(
            "Move the test_gap Budget to budgets.production.test_gap in {file}, or remove it."
        ))
    }
}

fn metric_budget_outside_production_remedy(
    file: ConfigFileKind,
    section: Option<&str>,
) -> Cow<'static, str> {
    if section == Some("new_file_budgets") {
        Cow::Owned(format!(
            "Move this Budget to new_file_budgets.production in {file}, or remove it."
        ))
    } else {
        Cow::Owned(format!(
            "Move this Budget to budgets.production in {file}, or remove it."
        ))
    }
}

fn sql_hazards_and_ratchet_config_remedy(
    error: &ConfigurationFileError,
) -> Option<Cow<'static, str>> {
    if let Some(remedy) = framework_profile_config_remedy(error) {
        return Some(remedy);
    }
    use ConfigurationFileError::*;
    Some(match error {
        EmptySqlDialectInclude { file, .. } => Cow::Owned(format!(
            "Add at least one glob pattern under include for every entry in sql.dialects in {file}."
        )),
        EmptySqlDialectPattern { file, .. } => Cow::Owned(format!(
            "Set non-empty glob patterns in sql.dialects in {file}, or remove the empty entry."
        )),
        EmptyHazardRuleName { file, .. } => Cow::Owned(format!(
            "Set non-empty rule names in hazards.disabled_rules in {file}, or remove the empty entry."
        )),
        EmptyRatchetPath { file, .. } => Cow::Owned(format!(
            "Set a non-empty repository-relative path in ratchet.path in {file}, or remove ratchet.path to use .codeint/ratchet.json."
        )),
        AbsoluteRatchetPath { file, .. } | ParentTraversalRatchetPath { file, .. } => Cow::Owned(
            format!("Use a repository-relative path without \"..\" in ratchet.path in {file}."),
        ),
        _ => return None,
    })
}

fn framework_profile_config_remedy(error: &ConfigurationFileError) -> Option<Cow<'static, str>> {
    use ConfigurationFileError::*;
    let (file, message) = match error {
        UnknownFrameworkProfile { file, .. } => (
            *file,
            "Use one of the known Framework profile names in {file}, or run codeint schema to list supported Framework profiles.",
        ),
        EmptyFrameworkProfileName { file, .. } => (
            *file,
            "Set non-empty Framework profile names in {file}, or remove the empty entry.",
        ),
        ConflictingFrameworkProfileRule { file, .. } => (
            *file,
            "Set either exclude or only on framework_profiles in {file}, not both.",
        ),
        EmptyFrameworkProfileObject { file, .. } => (
            *file,
            "Set either exclude or only in the framework_profiles object in {file}, or use false or a list of names.",
        ),
        InvalidPackageConfigKey { file, .. } => (
            *file,
            "Key packages in {file} by repository-relative Workspace package paths with forward slashes (or \".\" for the repository root).",
        ),
        DuplicatePackageConfigKey { file, .. } => (
            *file,
            "Merge duplicate Workspace package keys under packages in {file} so each package appears once.",
        ),
        _ => return None,
    };
    Some(Cow::Owned(message.replace("{file}", file.file_name())))
}

fn plugin_host_remedy(error: &PluginHostError, file: ConfigFileKind) -> Cow<'static, str> {
    plugin_load_remedy(error, file).unwrap_or_else(|| plugin_runtime_remedy(error, file))
}

fn plugin_load_remedy(error: &PluginHostError, file: ConfigFileKind) -> Option<Cow<'static, str>> {
    Some(match error {
        PluginHostError::Read { .. } => Cow::Owned(format!(
            "Check that the plugin path in {file} points to an existing readable WebAssembly component file."
        )),
        PluginHostError::HashMismatch { .. } | PluginHostError::DownloadHashMismatch { .. } => {
            Cow::Owned(format!(
                "Verify the plugin file has not been modified, then update sha256 in {file} to match the trusted file."
            ))
        }
        PluginHostError::Compile { .. } => Cow::Borrowed(
            "Build the plugin as a WebAssembly component targeting codeint:plugin@0.4.0.",
        ),
        PluginHostError::Instantiate { .. } => Cow::Borrowed(
            "Ensure the plugin exports codeint:plugin/language@0.4.0 or codeint:plugin/metric@0.4.0 and imports no host capabilities.",
        ),
        PluginHostError::IncompatibleContractVersion { .. } => Cow::Owned(format!(
            "Rebuild the plugin against codeint:plugin@0.4.0 and update its sha256 in {file}."
        )),
        PluginHostError::NonHttpsUrl { .. } => Cow::Owned(format!(
            "Use an https:// URL for the plugin in {file}; plain HTTP and other schemes are refused."
        )),
        PluginHostError::MissingCacheDir { .. } => Cow::Borrowed(
            "Set CODEINT_CACHE_DIR, XDG_CACHE_HOME, HOME, or LOCALAPPDATA so codeint can cache downloaded plugins.",
        ),
        PluginHostError::CacheWrite { .. } => Cow::Borrowed(
            "Check that you have permission to write to the codeint plugin cache directory, or set CODEINT_CACHE_DIR to a writable folder.",
        ),
        PluginHostError::Download { .. } => Cow::Borrowed(
            "Check that curl is installed, the HTTPS URL is reachable, and its TLS certificate is valid.",
        ),
        _ => return None,
    })
}

fn plugin_runtime_remedy(error: &PluginHostError, file: ConfigFileKind) -> Cow<'static, str> {
    match error {
        PluginHostError::Call { .. } => Cow::Borrowed(
            "Fix the defect in the external Plugin, or report it to the plugin's author.",
        ),
        PluginHostError::MemoryLimitExceeded { .. } | PluginHostError::TimeLimitExceeded { .. } => {
            Cow::Owned(format!(
                "Raise plugin_limits.memory_mb or plugin_limits.timeout_ms in {file} if the plugin needs more resources, or fix the defect in the plugin."
            ))
        }
        PluginHostError::DuplicatePluginName { .. } => Cow::Owned(format!(
            "Give each external plugin in {file} a unique name that does not collide with built-in plugins."
        )),
        PluginHostError::InvalidMetricKey { .. } => Cow::Borrowed(
            "Return a valid Metric key that does not collide with built-in report keys from the Metric plugin.",
        ),
        PluginHostError::DuplicateMetricKey { .. } => {
            Cow::Borrowed("Remove one of the Metric plugins that declare the same Metric key.")
        }
        PluginHostError::InvalidMetricVersion { .. } => Cow::Borrowed(
            "Return a positive integer Metric version (>= 1) from the Metric plugin's version export.",
        ),
        _ => Cow::Owned(format!("Check the plugin configuration in {file}.")),
    }
}

fn language_plugin_remedy(error: &LanguagePluginError, file: ConfigFileKind) -> Cow<'static, str> {
    match error {
        LanguagePluginError::MemoryLimitExceeded { .. }
        | LanguagePluginError::TimeLimitExceeded { .. } => Cow::Owned(format!(
            "Raise plugin_limits.memory_mb or plugin_limits.timeout_ms in {file} if the Language plugin needs more resources, or fix the defect in the plugin."
        )),
        LanguagePluginError::HazardLineOutOfRange { .. } => Cow::Borrowed(
            "Return 1-based Hazard line numbers within the file's line count from the Language plugin, or report the defect to its author.",
        ),
        LanguagePluginError::UnlistedHazardRule { .. } => Cow::Borrowed(
            "List every Hazard line rule name in the Language plugin's hazard-rules export, or report the defect to the plugin's author.",
        ),
        _ => Cow::Borrowed(
            "Fix the defect in the Language plugin, or report it to the plugin's author.",
        ),
    }
}

fn metric_plugin_remedy(error: &MetricPluginError, file: ConfigFileKind) -> Cow<'static, str> {
    match error {
        MetricPluginError::MemoryLimitExceeded { .. }
        | MetricPluginError::TimeLimitExceeded { .. } => Cow::Owned(format!(
            "Raise plugin_limits.memory_mb or plugin_limits.timeout_ms in {file} if the Metric plugin needs more resources, or fix the defect in the plugin."
        )),
        _ => Cow::Borrowed(
            "Fix the defect in the external Metric plugin, or report it to the plugin's author.",
        ),
    }
}

fn rerender_remedy(error: &RerenderError) -> &'static str {
    match error {
        RerenderError::ReadStdin(_) => {
            "Pipe a valid UTF-8 JSON report into standard input when passing -."
        }
        RerenderError::ReadFile { .. } => {
            "Pass a readable path to a saved check or diff JSON report, or pass - for standard input."
        }
        RerenderError::Parse(parse_error) => match parse_error {
            SavedReportError::InvalidJson(_) | SavedReportError::NotAnObject => {
                "Pass a JSON report generated by codeint check --format json or codeint diff --format json."
            }
            SavedReportError::UnsupportedSchemaVersion { .. } => {
                "Generate the JSON report with a compatible codeint version, or upgrade codeint."
            }
            SavedReportError::UnrecognizedReport | SavedReportError::InvalidPayload { .. } => {
                "Pass a JSON report generated by codeint check --format json or codeint diff --format json."
            }
        },
        RerenderError::Render(_) => {
            "This is a defect in codeint; report it at https://github.com/avidianity/codeint/issues."
        }
    }
}

fn starter_configuration_remedy(error: &StarterConfigurationError) -> Cow<'static, str> {
    match error {
        StarterConfigurationError::AlreadyExists { path } => {
            let label = ConfigFileKind::ALL
                .into_iter()
                .map(ConfigFileKind::file_name)
                .find(|name| path.ends_with(name))
                .unwrap_or("codeint.json");
            Cow::Owned(format!(
                "Edit the existing {label}, or delete it and run codeint init again."
            ))
        }
        StarterConfigurationError::Unwritable { .. } => Cow::Borrowed(
            "Check that you have permission to write to the root of the repository, then run codeint init again.",
        ),
    }
}

fn codeowners_remedy(error: &CodeownersError) -> &'static str {
    match error {
        CodeownersError::Read { .. } => {
            "Check that the CODEOWNERS file is readable UTF-8, then run codeint again."
        }
        CodeownersError::InvalidOwnerPattern { .. } => {
            "Pass a valid glob pattern to --owner, such as @org/team or @org/*."
        }
    }
}
