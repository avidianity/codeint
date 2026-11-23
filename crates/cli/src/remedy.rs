use codeint_config::{
    CodeownersError, ConfigError, ConfigurationFileError, StarterConfigurationError,
};
use codeint_git::GitError;
use codeint_metrics::ImpactError;
use codeint_model::{MetricPluginError, WorkspaceError};
use codeint_plugin_host::PluginHostError;
use codeint_report::TraceError;

use crate::CliError;
use crate::check::CheckError;
use crate::guard::GuardError;
use crate::impact::ImpactCommandError;
use crate::init::InitError;
use crate::trace::TraceCommandError;
use crate::update::UpdateError;

pub(crate) fn remedy(error: &CliError) -> &'static str {
    match error {
        CliError::Config(error) => config_remedy(error),
        CliError::Check(error) => check_remedy(error),
        CliError::Guard(error) => guard_remedy(error),
        CliError::Impact(error) => impact_remedy(error),
        CliError::Trace(error) => trace_remedy(error),
        CliError::Init(error) => init_remedy(error),
        CliError::Update(error) => update_remedy(error),
        CliError::Report(_) => {
            "This is a defect in codeint; report it at https://github.com/avidianity/codeint/issues."
        }
        CliError::Output(_) => "Keep standard output open until codeint exits.",
        CliError::OutputFile { .. } => {
            "Pass --output a file path that you have permission to write, not a folder."
        }
    }
}

fn trace_remedy(error: &TraceCommandError) -> &'static str {
    match error {
        TraceCommandError::Check(error) => check_remedy(error),
        TraceCommandError::UnknownModule { .. }
        | TraceCommandError::Trace(TraceError::UnknownModule { .. }) => {
            "Pass repository-relative paths of files measured by codeint (not ignored, unparsable, declaration-only, or outside the repository)."
        }
        TraceCommandError::Trace(TraceError::InvalidMaxPaths) => {
            "Pass a positive integer (at least 1) for --max-paths."
        }
    }
}

fn check_remedy(error: &CheckError) -> &'static str {
    match error {
        CheckError::ExtensionConflict { .. } => {
            "Assign separate directories to each Language plugin in the languages setting of codeint.json."
        }
        CheckError::UnknownWorkspacePackage { .. } => {
            "Pass one of the discovered Workspace package paths to --package, or omit --package to measure the whole repository."
        }
        CheckError::Git(error) => git_remedy(error),
        CheckError::Configuration(error) => configuration_file_remedy(error),
        CheckError::Codeowners(error) => codeowners_remedy(error),
        CheckError::Workspace(error) => workspace_remedy(error),
        CheckError::Resolve(_) | CheckError::LanguagePlugin(_) => {
            "This is a defect in the Language plugin; report it to the author of the Language plugin."
        }
        CheckError::ReferenceRange(_) => {
            "Fix the Reference ranges that the Language plugin supplies, or report the defect to its author."
        }
        CheckError::PluginHost(error) => plugin_host_remedy(error),
        CheckError::MetricPlugin(error) => metric_plugin_remedy(error),
    }
}

fn guard_remedy(error: &GuardError) -> &'static str {
    match error {
        GuardError::Git(error) => git_remedy(error),
        GuardError::Configuration(error) => configuration_file_remedy(error),
        GuardError::OutsideRepository { .. } => {
            "Pass a file path inside the Git repository, such as src/domain/service.ts."
        }
        GuardError::NonUtf8Path { .. } | GuardError::InvalidPath { .. } => {
            "Pass a valid UTF-8 file path inside the Git repository."
        }
    }
}

fn impact_remedy(error: &ImpactCommandError) -> &'static str {
    match error {
        ImpactCommandError::Check(error) => check_remedy(error),
        ImpactCommandError::Analysis(ImpactError::TargetNotInCodeModel { .. }) => {
            "Pass a tracked or non-ignored source file that a Language plugin measures, or check the file path."
        }
        ImpactCommandError::MissingTargets => {
            "Pass one or more repository file paths, or pass --changed <base> to analyze changed files."
        }
        ImpactCommandError::PathOutsideRepository { .. } => {
            "Pass a file path inside the Git repository."
        }
        ImpactCommandError::InvalidTargetPath { .. } => {
            "Pass a repository-relative file path using forward slashes."
        }
    }
}

fn init_remedy(error: &InitError) -> &'static str {
    match error {
        InitError::Git(GitError::NotInRepository { .. }) => {
            "Run codeint init inside a Git repository; it writes codeint.json at the root of the repository."
        }
        InitError::Git(error) => git_remedy(error),
        InitError::StarterConfiguration(error) => starter_configuration_remedy(error),
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
    }
}

fn git_remedy(error: &GitError) -> &'static str {
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
        WorkspaceError::InvalidPackageRoot(_) => {
            "This is a defect in the Language plugin; report it to the Plugin's author."
        }
    }
}

fn configuration_file_remedy(error: &ConfigurationFileError) -> &'static str {
    plugin_config_remedy(error)
        .or_else(|| boundary_config_remedy(error))
        .or_else(|| file_access_remedy(error))
        .or_else(|| json_shape_remedy(error))
        .unwrap_or("Check codeint.json against the JSON schema that $schema links to.")
}

fn file_access_remedy(error: &ConfigurationFileError) -> Option<&'static str> {
    use ConfigurationFileError as E;
    match error {
        E::Unreadable(_) => Some(
            "Check that codeint.json is a file you have permission to read, then run codeint again.",
        ),
        E::NotJson(_) => Some("Fix the JSON syntax of codeint.json at that line and column."),
        E::NewerSchemaVersion { .. } => Some(
            "Run codeint update (or upgrade the codeint npm package) to match or exceed the $schema version, or lower the $schema tag in codeint.json.",
        ),
        _ => None,
    }
}

fn json_shape_remedy(error: &ConfigurationFileError) -> Option<&'static str> {
    use ConfigurationFileError::*;
    Some(match error {
        UnknownKey { .. } => "Remove the key, or correct its spelling to one of the allowed keys.",
        WrongType { .. } => {
            "Change the value to the expected type; the JSON schema that $schema links to describes every key."
        }
        OutOfRange {
            minimum, maximum, ..
        } => out_of_range_remedy(*minimum, *maximum),
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

fn boundary_config_remedy(error: &ConfigurationFileError) -> Option<&'static str> {
    use ConfigurationFileError::*;
    match error {
        InvalidBoundaryZoneName { .. } => {
            Some("Give each boundary zone in codeint.json a non-empty name other than \"*\".")
        }
        EmptyBoundaryZonePatterns { .. } => {
            Some("Add at least one path glob under patterns for every zone in boundaries.zones.")
        }
        UnknownBoundaryZone { .. } => Some(
            "Reference only zones defined under boundaries.zones (or \"*\") in boundaries.rules.",
        ),
        ConflictingBoundaryRule { .. } => {
            Some("Set either allow or deny on each rule in boundaries.rules, not both.")
        }
        MissingBoundaryRuleDirective { .. } => {
            Some("Set either allow or deny on each rule in boundaries.rules.")
        }
        ConflictingBoundaryDirectives { .. } => Some(
            "Use either allow rules or deny rules for a given source zone in boundaries.rules, not both.",
        ),
        _ => None,
    }
}

fn plugin_config_remedy(error: &ConfigurationFileError) -> Option<&'static str> {
    use ConfigurationFileError::*;
    Some(match error {
        UnknownLanguagePlugin { .. } => {
            "Name one of the Language plugins listed, or remove the entry from languages."
        }
        InvalidGlob { .. } => {
            "Fix the glob: * matches within one folder, ** matches across folders, and {a,b} matches either a or b."
        }
        InvalidSha256 { .. } => {
            "Set sha256 to the 64-character hexadecimal SHA-256 digest of the plugin file."
        }
        EmptyPluginField { .. } => "Set a non-empty value for the plugin field in codeint.json.",
        MissingPluginSource { .. } => {
            "Set either path (for a local file) or url (for an https:// download) on the plugin entry in codeint.json."
        }
        AmbiguousPluginSource { .. } => {
            "Keep either path or url on the plugin entry in codeint.json and remove the other."
        }
        NonHttpsPluginUrl { .. } => {
            "Use an https:// URL for the plugin in codeint.json; plain HTTP and other schemes are refused."
        }
        EmptyDependencyIgnore { .. } => {
            "Set non-empty package names in dependencies.ignore in codeint.json, or remove the empty entry."
        }
        _ => return None,
    })
}

fn plugin_host_remedy(error: &PluginHostError) -> &'static str {
    plugin_load_remedy(error).unwrap_or_else(|| plugin_runtime_remedy(error))
}

fn plugin_load_remedy(error: &PluginHostError) -> Option<&'static str> {
    Some(match error {
        PluginHostError::Read { .. } => {
            "Check that the plugin path in codeint.json points to an existing readable WebAssembly component file."
        }
        PluginHostError::HashMismatch { .. } | PluginHostError::DownloadHashMismatch { .. } => {
            "Verify the plugin file has not been modified, then update sha256 in codeint.json to match the trusted file."
        }
        PluginHostError::Compile { .. } => {
            "Build the plugin as a WebAssembly component targeting codeint:plugin@0.2.0."
        }
        PluginHostError::Instantiate { .. } => {
            "Ensure the plugin exports codeint:plugin/language@0.2.0 or codeint:plugin/metric@0.2.0 and imports no host capabilities."
        }
        PluginHostError::IncompatibleContractVersion { .. } => {
            "Rebuild the plugin against codeint:plugin@0.2.0 and update its sha256 in codeint.json."
        }
        PluginHostError::NonHttpsUrl { .. } => {
            "Use an https:// URL for the plugin in codeint.json; plain HTTP and other schemes are refused."
        }
        PluginHostError::MissingCacheDir { .. } => {
            "Set CODEINT_CACHE_DIR, XDG_CACHE_HOME, HOME, or LOCALAPPDATA so codeint can cache downloaded plugins."
        }
        PluginHostError::CacheWrite { .. } => {
            "Check that you have permission to write to the codeint plugin cache directory, or set CODEINT_CACHE_DIR to a writable folder."
        }
        PluginHostError::Download { .. } => {
            "Check that curl is installed, the HTTPS URL is reachable, and its TLS certificate is valid."
        }
        _ => return None,
    })
}

fn plugin_runtime_remedy(error: &PluginHostError) -> &'static str {
    match error {
        PluginHostError::Call { .. } => {
            "Fix the defect in the external Plugin, or report it to the plugin's author."
        }
        PluginHostError::MemoryLimitExceeded { .. } | PluginHostError::TimeLimitExceeded { .. } => {
            "Raise plugin_limits.memory_mb or plugin_limits.timeout_ms in codeint.json if the plugin needs more resources, or fix the defect in the plugin."
        }
        PluginHostError::DuplicatePluginName { .. } => {
            "Give each external plugin in codeint.json a unique name that does not collide with built-in plugins."
        }
        PluginHostError::InvalidMetricKey { .. } => {
            "Return a valid Metric key that does not collide with built-in report keys from the Metric plugin."
        }
        PluginHostError::DuplicateMetricKey { .. } => {
            "Remove one of the Metric plugins that declare the same Metric key."
        }
        _ => "Check the plugin configuration in codeint.json.",
    }
}

fn metric_plugin_remedy(error: &MetricPluginError) -> &'static str {
    match error {
        MetricPluginError::MemoryLimitExceeded { .. }
        | MetricPluginError::TimeLimitExceeded { .. } => {
            "Raise plugin_limits.memory_mb or plugin_limits.timeout_ms in codeint.json if the Metric plugin needs more resources, or fix the defect in the plugin."
        }
        _ => "Fix the defect in the external Metric plugin, or report it to the plugin's author.",
    }
}

fn starter_configuration_remedy(error: &StarterConfigurationError) -> &'static str {
    match error {
        StarterConfigurationError::AlreadyExists { .. } => {
            "Edit the existing codeint.json, or delete it and run codeint init again."
        }
        StarterConfigurationError::Unwritable { .. } => {
            "Check that you have permission to write to the root of the repository, then run codeint init again."
        }
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
