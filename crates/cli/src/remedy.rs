use codeint_config::ConfigError;
use codeint_git::GitError;

use crate::CliError;
use crate::check::CheckError;

pub(crate) fn remedy(error: &CliError) -> &'static str {
    match error {
        CliError::Config(error) => config_remedy(error),
        CliError::Check(CheckError::Git(error)) => git_remedy(error),
        CliError::Check(CheckError::NotUtf8 { .. }) => {
            "Save the file as UTF-8, then run codeint again."
        }
        CliError::Check(CheckError::Report(_)) => {
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
    }
}
