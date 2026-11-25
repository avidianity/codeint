use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::command::{run_checked, stdout_text};
use crate::error::{EvaluationError, io_error};
use crate::recall::OracleFile;

const PYTHON_ORACLE: &str = include_str!("../oracles/python/oracle.py");
const GO_ORACLE: &str = include_str!("../oracles/go/oracle.go");
const PHP_ORACLE: &str = include_str!("../oracles/php/oracle.php");
const DART_PUBSPEC: &str = include_str!("../oracles/dart/pubspec.yaml");
const DART_ORACLE: &str = include_str!("../oracles/dart/bin/oracle.dart");

struct Runtime {
    language: &'static str,
    program: &'static str,
    version_args: &'static [&'static str],
    minimum: (u32, u32),
}

const RUNTIMES: [Runtime; 4] = [
    Runtime {
        language: "python",
        program: "python3",
        version_args: &["--version"],
        minimum: (3, 12),
    },
    Runtime {
        language: "go",
        program: "go",
        version_args: &["version"],
        minimum: (1, 24),
    },
    Runtime {
        language: "php",
        program: "php",
        version_args: &["-r", "echo PHP_VERSION;"],
        minimum: (8, 1),
    },
    Runtime {
        language: "dart",
        program: "dart",
        version_args: &["--version"],
        minimum: (3, 11),
    },
];

pub(crate) struct OracleCommand {
    program: PathBuf,
    args: Vec<OsString>,
}

pub(crate) enum PreparedOracle {
    Ready(OracleCommand),
    Unavailable(String),
}

pub(crate) fn prepare_oracles<'language>(
    languages: impl IntoIterator<Item = &'language str>,
    workspace: &Path,
    overrides: &BTreeMap<String, PathBuf>,
) -> BTreeMap<String, PreparedOracle> {
    languages
        .into_iter()
        .filter_map(|language| {
            let prepared = match overrides.get(language) {
                Some(program) => PreparedOracle::Ready(OracleCommand {
                    program: program.clone(),
                    args: Vec::new(),
                }),
                None => prepare_runtime(
                    RUNTIMES
                        .iter()
                        .find(|runtime| runtime.language == language)?,
                    workspace,
                ),
            };
            Some((language.to_owned(), prepared))
        })
        .collect()
}

fn prepare_runtime(runtime: &Runtime, workspace: &Path) -> PreparedOracle {
    if let Err(reason) = check_version(runtime) {
        return PreparedOracle::Unavailable(reason);
    }
    let folder = workspace.join(runtime.language);
    match build_oracle(runtime.language, &folder) {
        Ok(command) => PreparedOracle::Ready(command),
        Err(error) => PreparedOracle::Unavailable(error.to_string()),
    }
}

pub(crate) fn parse_version(text: &str) -> Option<(u32, u32)> {
    let start = text.find(|character: char| character.is_ascii_digit())?;
    let mut parts = text[start..]
        .split(|character: char| !character.is_ascii_digit())
        .map(str::parse::<u32>);
    Some((parts.next()?.ok()?, parts.next()?.ok()?))
}

fn check_version(runtime: &Runtime) -> Result<(), String> {
    let text =
        stdout_text(Command::new(runtime.program).args(runtime.version_args)).map_err(|error| {
            format!(
                "{} {}.{}+ is not available: {error}",
                runtime.program, runtime.minimum.0, runtime.minimum.1
            )
        })?;
    let version = parse_version(&text)
        .ok_or_else(|| format!("cannot read the {} version from {text:?}", runtime.program))?;
    if version < runtime.minimum {
        return Err(format!(
            "{} {}.{} is older than the required {}.{}",
            runtime.program, version.0, version.1, runtime.minimum.0, runtime.minimum.1
        ));
    }
    Ok(())
}

fn write_source(path: &Path, text: &str) -> Result<(), EvaluationError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(io_error(parent))?;
    }
    std::fs::write(path, text).map_err(io_error(path))
}

fn interpreted(program: &str, script: PathBuf) -> OracleCommand {
    OracleCommand {
        program: PathBuf::from(program),
        args: vec![script.into_os_string()],
    }
}

fn build_oracle(language: &str, folder: &Path) -> Result<OracleCommand, EvaluationError> {
    match language {
        "python" => {
            write_source(&folder.join("oracle.py"), PYTHON_ORACLE)?;
            Ok(interpreted("python3", folder.join("oracle.py")))
        }
        "php" => {
            write_source(&folder.join("oracle.php"), PHP_ORACLE)?;
            Ok(interpreted("php", folder.join("oracle.php")))
        }
        "go" => build_go_oracle(folder),
        _ => build_dart_oracle(folder),
    }
}

fn compiled(program: PathBuf) -> OracleCommand {
    OracleCommand {
        program,
        args: Vec::new(),
    }
}

fn build_go_oracle(folder: &Path) -> Result<OracleCommand, EvaluationError> {
    let source = folder.join("oracle.go");
    write_source(&source, GO_ORACLE)?;
    let binary = folder.join("oracle");
    run_checked(
        Command::new("go")
            .arg("build")
            .arg("-o")
            .arg(&binary)
            .arg(&source)
            .current_dir(folder),
    )?;
    Ok(compiled(binary))
}

fn build_dart_oracle(folder: &Path) -> Result<OracleCommand, EvaluationError> {
    write_source(&folder.join("pubspec.yaml"), DART_PUBSPEC)?;
    write_source(&folder.join("bin/oracle.dart"), DART_ORACLE)?;
    run_checked(
        Command::new("dart")
            .args(["pub", "get"])
            .env("PUB_CACHE", folder.join(".pub-cache"))
            .current_dir(folder),
    )?;
    let binary = folder.join("oracle");
    run_checked(
        Command::new("dart")
            .args(["compile", "exe", "bin/oracle.dart", "-o"])
            .arg(&binary)
            .env("PUB_CACHE", folder.join(".pub-cache"))
            .current_dir(folder),
    )?;
    Ok(compiled(binary))
}

pub(crate) fn run_oracle(
    oracle: &OracleCommand,
    checkout: &Path,
    scratch: &Path,
    paths: &[&str],
) -> Result<Vec<OracleFile>, EvaluationError> {
    let listing = scratch.join("oracle-files.txt");
    let mut text = String::new();
    for path in paths {
        text.push_str(path);
        text.push('\0');
    }
    std::fs::write(&listing, text).map_err(io_error(&listing))?;
    let output = run_checked(
        Command::new(&oracle.program)
            .args(&oracle.args)
            .arg(&listing)
            .current_dir(checkout),
    )?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line).map_err(|error| EvaluationError::Command {
                command: oracle.program.display().to_string(),
                detail: format!("printed a line that is not an oracle record: {error}"),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
