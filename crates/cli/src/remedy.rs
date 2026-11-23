use codeint_config::{ConfigError, ConfigurationFileError, StarterConfigurationError};
use codeint_git::GitError;
use codeint_model::{MetricPluginError, WorkspaceError};
use codeint_plugin_host::PluginHostError;

use crate::CliError;
use crate::check::CheckError;
use crate::init::InitError;
use crate::update::UpdateError;

pub(crate) fn remedy(error: &CliError) -> &'static str {
    match error {
        CliError::Config(error) => config_remedy(error),
        CliError::Check(CheckError::Git(error)) => git_remedy(error),
        CliError::Check(CheckError::Configuration(error)) => configuration_file_remedy(error),
        CliError::Check(CheckError::Workspace(error)) => workspace_remedy(error),
        CliError::Check(CheckError::Resolve(_)) => {
            "This is a defect in the Language plugin; report it to the author of the Language plugin."
        }
        CliError::Check(CheckError::ReferenceRange(_)) => {
            "Fix the Reference ranges that the Language plugin supplies, or report the defect to its author."
        }
        CliError::Check(CheckError::PluginHost(error)) => plugin_host_remedy(error),
        CliError::Check(CheckError::MetricPlugin(error)) => metric_plugin_remedy(error),
        CliError::Check(CheckError::ExtensionConflict { .. }) => {
            "Assign separate directories to each Language plugin in the languages setting of codeint.json."
        }
        CliError::Init(InitError::Git(GitError::NotInRepository { .. })) => {
            "Run codeint init inside a Git repository; it writes codeint.json at the root of the repository."
        }
        CliError::Init(InitError::Git(error)) => git_remedy(error),
        CliError::Init(InitError::StarterConfiguration(error)) => {
            starter_configuration_remedy(error)
        }
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
            "Commit once, then run codeint trend again; it measures the commits of the current branch."
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
    if let Some(message) = plugin_config_remedy(error) {
        return message;
    }
    match error {
        ConfigurationFileError::Unreadable(_) => {
            "Check that codeint.json is a file you have permission to read, then run codeint again."
        }
        ConfigurationFileError::NotJson(_) => {
            "Fix the JSON syntax of codeint.json at that line and column."
        }
        ConfigurationFileError::UnknownKey { .. } => {
            "Remove the key, or correct its spelling to one of the allowed keys."
        }
        ConfigurationFileError::WrongType { .. } => {
            "Change the value to the expected type; the JSON schema that $schema links to describes every key."
        }
        ConfigurationFileError::OutOfRange {
            minimum, maximum, ..
        } if *minimum == 0.0 && *maximum == 1.0 => {
            "Set the value to a ratio from 0 to 1, as the report shows ratios."
        }
        ConfigurationFileError::OutOfRange { .. } => {
            "Set the value within the allowed range; the JSON schema that $schema links to describes every key."
        }
        ConfigurationFileError::NotOneOf { .. } => {
            "Set the value to one of the allowed values; the JSON schema that $schema links to describes each one."
        }
        ConfigurationFileError::MissingKey { .. } => {
            "Add the key; the JSON schema that $schema links to describes every key."
        }
        ConfigurationFileError::NewerSchemaVersion { .. } => {
            "Run codeint update (or upgrade the codeint npm package) to match or exceed the $schema version, or lower the $schema tag in codeint.json."
        }
        ConfigurationFileError::InvalidBoundaryZoneName { .. } => {
            "Give each boundary zone in codeint.json a non-empty name other than \"*\"."
        }
        ConfigurationFileError::EmptyBoundaryZonePatterns { .. } => {
            "Add at least one path glob under patterns for every zone in boundaries.zones."
        }
        ConfigurationFileError::UnknownBoundaryZone { .. } => {
            "Reference only zones defined under boundaries.zones (or \"*\") in boundaries.rules."
        }
        ConfigurationFileError::ConflictingBoundaryRule { .. } => {
            "Set either allow or deny on each rule in boundaries.rules, not both."
        }
        ConfigurationFileError::MissingBoundaryRuleDirective { .. } => {
            "Set either allow or deny on each rule in boundaries.rules."
        }
        ConfigurationFileError::ConflictingBoundaryDirectives { .. } => {
            "Use either allow rules or deny rules for a given source zone in boundaries.rules, not both."
        }
        _ => "Check codeint.json against the JSON schema that $schema links to.",
    }
}

fn plugin_config_remedy(error: &ConfigurationFileError) -> Option<&'static str> {
    Some(match error {
        ConfigurationFileError::UnknownLanguagePlugin { .. } => {
            "Name one of the Language plugins listed, or remove the entry from languages."
        }
        ConfigurationFileError::InvalidGlob { .. } => {
            "Fix the glob: * matches within one folder, ** matches across folders, and {a,b} matches either a or b."
        }
        ConfigurationFileError::InvalidSha256 { .. } => {
            "Set sha256 to the 64-character hexadecimal SHA-256 digest of the plugin file."
        }
        ConfigurationFileError::EmptyPluginField { .. } => {
            "Set a non-empty value for the plugin field in codeint.json."
        }
        ConfigurationFileError::MissingPluginSource { .. } => {
            "Set either path (for a local file) or url (for an https:// download) on the plugin entry in codeint.json."
        }
        ConfigurationFileError::AmbiguousPluginSource { .. } => {
            "Keep either path or url on the plugin entry in codeint.json and remove the other."
        }
        ConfigurationFileError::NonHttpsPluginUrl { .. } => {
            "Use an https:// URL for the plugin in codeint.json; plain HTTP and other schemes are refused."
        }
        _ => return None,
    })
}

fn plugin_host_remedy(error: &PluginHostError) -> &'static str {
    match error {
        PluginHostError::Read { .. } => {
            "Check that the plugin path in codeint.json points to an existing readable WebAssembly component file."
        }
        PluginHostError::HashMismatch { .. } | PluginHostError::DownloadHashMismatch { .. } => {
            "Verify the plugin file has not been modified, then update sha256 in codeint.json to match the trusted file."
        }
        PluginHostError::Compile { .. } => {
            "Build the plugin as a WebAssembly component targeting codeint:plugin@0.1.0."
        }
        PluginHostError::Instantiate { .. } => {
            "Ensure the plugin exports codeint:plugin/language@0.1.0 or codeint:plugin/metric@0.1.0 and imports no host capabilities."
        }
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
