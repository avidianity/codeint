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
    #[arg(
        long,
        help = "Read the reviewer's verdicts on the audit sample from FILE and report false positive rates"
    )]
    audit_verdicts: Option<PathBuf>,
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

fn codeint_checkout() -> PathBuf {
    let manifest_folder = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_folder
        .parent()
        .unwrap_or(manifest_folder)
        .to_path_buf()
}

fn protected_repositories(manifest: &Path, current: &Path) -> Vec<PathBuf> {
    let manifest_repository = manifest
        .canonicalize()
        .ok()
        .and_then(|path| path.parent().and_then(toplevel));
    let mut repositories = vec![codeint_checkout()];
    repositories.extend(manifest_repository);
    repositories.extend(toplevel(current));
    repositories
}

pub(crate) fn evaluation_config() -> Result<EvaluationConfig, std::io::Error> {
    let arguments = Arguments::parse();
    let protected_repositories =
        protected_repositories(&arguments.manifest, &std::env::current_dir()?);
    Ok(EvaluationConfig {
        manifest: arguments.manifest,
        codeint: arguments.codeint,
        output_dir: arguments.output_dir,
        protected_repositories,
        scratch_root: arguments.scratch_root,
        case_studies: arguments.language.is_empty() && arguments.repository.is_empty(),
        languages: arguments.language,
        repositories: arguments.repository,
        runs: arguments.runs,
        audit_sample_size: arguments.audit_sample_size,
        oracle_overrides: arguments.oracle.into_iter().collect::<BTreeMap<_, _>>(),
        audit_verdicts: arguments.audit_verdicts,
    })
}
