use codeint_config::{ConfigError, ConfigurationFileError, StarterConfigurationError};
use codeint_git::GitError;
use codeint_model::WorkspaceError;

use crate::CliError;
use crate::check::CheckError;
use crate::init::InitError;

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
        CliError::Init(InitError::Git(GitError::NotInRepository { .. })) => {
            "Run codeint init inside a Git repository; it writes codeint.json at the root of the repository."
        }
        CliError::Init(InitError::Git(error)) => git_remedy(error),
        CliError::Init(InitError::StarterConfiguration(error)) => {
            starter_configuration_remedy(error)
        }
        CliError::Report(_) => {
            "This is a defect in codeint; report it at https://github.com/avidianity/codeint/issues."
        }
        CliError::Output(_) => "Keep standard output open until codeint exits.",
        CliError::OutputFile { .. } => {
            "Pass --output a file path that you have permission to write, not a folder."
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
        GitError::MergeBase { .. } | GitError::CommitTree { .. } | GitError::ReadBlob { .. } => {
            "Check that git log and git show work in this repository, then run codeint again."
        }
    }
}

fn workspace_remedy(error: &WorkspaceError) -> &'static str {
    match error {
        WorkspaceError::Manifest { .. } => {
            "Fix the workspaces field of the manifest so that your package manager accepts it, then run codeint again."
        }
        WorkspaceError::InvalidPackageRoot(_) => {
            "This is a defect in the Language plugin; report it to the Plugin's author."
        }
    }
}

fn configuration_file_remedy(error: &ConfigurationFileError) -> &'static str {
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
        ConfigurationFileError::OutOfRange { .. } => {
            "Set the Budget to a ratio from 0 to 1, as the report shows the Metric."
        }
        ConfigurationFileError::MissingKey { .. } => {
            "Add the key; the JSON schema that $schema links to describes every key."
        }
        ConfigurationFileError::UnknownLanguagePlugin { .. } => {
            "Name one of the Language plugins listed, or remove the entry from languages."
        }
        ConfigurationFileError::InvalidGlob { .. } => {
            "Fix the glob: * matches within one folder, ** matches across folders, and {a,b} matches either a or b."
        }
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
