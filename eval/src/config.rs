use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Parser;
use codeint_eval::EvaluationConfig;

#[derive(Parser)]
#[command(
    name = "codeint-eval",
    about = "Measure the pinned Evaluation suite corpus in scratch folders outside the repository"
)]
struct Arguments {
    #[arg(
        long,
        default_value = "eval/corpus.json",
        help = "The pinned corpus manifest"
    )]
    manifest: PathBuf,
    #[arg(
        long,
        default_value = "target/release/codeint",
        help = "The codeint binary to measure"
    )]
    codeint: PathBuf,
    #[arg(
        long,
        default_value = "docs",
        help = "Where to write evaluation.md and evaluation-summary.json"
    )]
    output_dir: PathBuf,
    #[arg(
        long,
        help = "Measure only repositories of this Language plugin; may repeat"
    )]
    language: Vec<String>,
    #[arg(long, help = "Measure only the repository with this name; may repeat")]
    repository: Vec<String>,
    #[arg(
        long,
        default_value_t = 3,
        help = "How many times to run codeint check per repository"
    )]
    runs: u32,
    #[arg(
        long,
        default_value_t = 100,
        help = "How many Findings to sample per audit category"
    )]
    audit_sample_size: usize,
    #[arg(
        long,
        help = "Create scratch folders under DIR instead of the system temporary folder"
    )]
    scratch_root: Option<PathBuf>,
    #[arg(long, value_parser = parse_oracle, help = "Use PROGRAM as the oracle for LANGUAGE, as LANGUAGE=PROGRAM; may repeat")]
    oracle: Vec<(String, PathBuf)>,
}

fn parse_oracle(value: &str) -> Result<(String, PathBuf), String> {
    value
        .split_once('=')
        .map(|(language, program)| (language.to_owned(), PathBuf::from(program)))
        .ok_or_else(|| format!("expected LANGUAGE=PROGRAM, not {value}"))
}

fn toplevel(folder: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(folder)
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let text = String::from_utf8(output.stdout).ok()?;
    Some(PathBuf::from(text.trim_end_matches(['\r', '\n'])))
}

fn repository_root(manifest: &Path, current: PathBuf) -> PathBuf {
    let manifest_folder = manifest
        .canonicalize()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf));
    manifest_folder
        .and_then(|folder| toplevel(&folder))
        .or_else(|| toplevel(&current))
        .unwrap_or(current)
}

pub(crate) fn evaluation_config() -> Result<EvaluationConfig, std::io::Error> {
    let arguments = Arguments::parse();
    let codeint_repository = repository_root(&arguments.manifest, std::env::current_dir()?);
    Ok(EvaluationConfig {
        manifest: arguments.manifest,
        codeint: arguments.codeint,
        output_dir: arguments.output_dir,
        codeint_repository,
        scratch_root: arguments.scratch_root,
        languages: arguments.language,
        repositories: arguments.repository,
        runs: arguments.runs,
        audit_sample_size: arguments.audit_sample_size,
        oracle_overrides: arguments.oracle.into_iter().collect::<BTreeMap<_, _>>(),
    })
}
