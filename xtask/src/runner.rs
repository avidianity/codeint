use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::checks::budget::{self, FeedbackLoop, Verdict};
use crate::checks::{
    Violations, duplicates, layout, perf, pinning, pull_request, release, source, tier,
};
use crate::config::{self, Settings};
use crate::git::{self, DiffSource};

const USAGE: &str = "usage: cargo xtask {command}

Whole-repository checks:
  agent-entry-links | skills-link | doc-size | doc-style | no-placeholders
  record-numbers | dependency-pinning | config-access | no-suppressions
  version-sync [--tag {tag}]
  no-duplicate-code        run jscpd and report clones outside test input
  merged-result            run every whole-repository check

Change checks:
  no-comments --staged | --base {ref}
  commit-message --file {path} | --range {base}
  behavior-has-test --base {ref} --body-file {path}
  evidence-bundle --body-file {path}
  risk-tier --base {ref}

Release:
  release-notes [--since {ref}] [--output {path}]
  prepare-release --version {x.y.z} [--since {ref}] [--dry-run]

Loops:
  hook                     the pre-commit checks on staged files
  affected-tests [--base {ref}]
  full                     every check and every test
  feedback-budget --loop {hook|relevant|ci-gate} (--seconds {n} | --run-id {id} --exclude-job {name}) [--report]
  perf-check [--binary {path}]";

pub(crate) fn run(command: &str, args: &[String]) -> Result<Violations, String> {
    let root = git::repository_root()?;
    let settings = config::load_settings(&root)?;
    match command {
        "agent-entry-links" => layout::agent_entry_links(&root, &settings),
        "skills-link" => layout::skills_link(&root, &settings),
        "doc-size" => layout::doc_size(&root, &settings),
        "doc-style" => layout::doc_style(&root, &settings),
        "no-placeholders" => layout::no_placeholders(&root, &settings),
        "record-numbers" => layout::record_numbers(&root, &settings),
        "dependency-pinning" => pinning::dependency_pinning(&root),
        "config-access" => source::config_access(&root, &settings),
        "no-suppressions" => source::no_suppressions(&root, &settings),
        "version-sync" => version_sync(&root, args),
        "release-notes" => release_notes(&root, args),
        "prepare-release" => prepare_release(&root, args),
        "no-duplicate-code" => duplicates::no_duplicate_code(&root, &settings),
        "no-comments" => source::no_comments(&root, &settings, &DiffSource::from_args(args)?),
        "commit-message" => commit_message(&root, &settings, args),
        "behavior-has-test" => behavior_has_test(&root, &settings, args),
        "evidence-bundle" => Ok(pull_request::evidence_bundle_problems(&read_file(&flag(
            args,
            "--body-file",
        )?)?)),
        "risk-tier" => risk_tier(&root, &settings, args),
        "merged-result" => merged_result(&root, &settings),
        "hook" => hook(&root, &settings),
        "affected-tests" => affected_tests(&root, args),
        "full" => full(&root),
        "feedback-budget" => feedback_budget(&root, &settings, args),
        "perf-check" => perf_check(&root, &settings, args),
        _ => Err(USAGE.to_string()),
    }
}

fn flag(args: &[String], name: &str) -> Result<String, String> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .cloned()
        .ok_or_else(|| format!("missing {name} {{value}}"))
}

fn read_file(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("cannot read {path}: {error}"))
}

fn commit_message(root: &Path, settings: &Settings, args: &[String]) -> Result<Violations, String> {
    let messages = if let Ok(path) = flag(args, "--file") {
        let merging = git::run(root, &["rev-parse", "-q", "--verify", "MERGE_HEAD"]).is_ok();
        vec![(
            "message".to_string(),
            pull_request::without_git_comments(&read_file(&path)?),
            merging,
        )]
    } else {
        let base = flag(args, "--range")?;
        git::run(
            root,
            &[
                "rev-list",
                "--reverse",
                "--parents",
                &format!("{base}..HEAD"),
            ],
        )?
        .lines()
        .map(|line| {
            let mut ids = line.split_whitespace();
            let commit = ids.next().unwrap_or_default();
            let is_merge = ids.count() > 1;
            Ok((
                commit[..commit.len().min(10)].to_string(),
                git::run(root, &["log", "-1", "--format=%B", commit])?,
                is_merge,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?
    };
    Ok(messages
        .iter()
        .flat_map(|(label, message, is_merge)| {
            pull_request::commit_message_problems(
                message,
                *is_merge,
                &settings.commit_types,
                &settings.commit_scopes,
            )
            .into_iter()
            .map(move |problem| format!("{label}: {problem}"))
        })
        .collect())
}

fn behavior_has_test(
    root: &Path,
    settings: &Settings,
    args: &[String],
) -> Result<Violations, String> {
    let source = DiffSource::Range(flag(args, "--base")?);
    let body = read_file(&flag(args, "--body-file")?)?;
    let changes = git::changed_files(root, &source)?;
    pull_request::behavior_has_test_problems(&changes, &body, settings)
}

fn risk_tier(root: &Path, settings: &Settings, args: &[String]) -> Result<Violations, String> {
    let source = DiffSource::Range(flag(args, "--base")?);
    let findings = tier::risk_tier(root, &source, settings)?;
    let highest = tier::highest_tier(&findings);
    println!(
        "Tier {highest}. See docs/agents/review-and-merge.md for the approval this Tier needs."
    );
    for finding in findings.iter().filter(|finding| finding.tier == highest) {
        println!(
            "Tier {}: {} {}.",
            finding.tier, finding.path, finding.reason
        );
    }
    if let Some(output) = config::github_output_file() {
        let mut existing = fs::read_to_string(&output).unwrap_or_default();
        existing.push_str(&format!("tier={highest}\n"));
        fs::write(&output, existing)
            .map_err(|error| format!("cannot write the GitHub output file: {error}"))?;
    }
    Ok(Vec::new())
}

type Check = fn(&Path, &Settings) -> Result<Violations, String>;

fn whole_repository_checks() -> Vec<(&'static str, Check)> {
    vec![
        ("agent-entry-links", layout::agent_entry_links),
        ("skills-link", layout::skills_link),
        ("doc-size", layout::doc_size),
        ("doc-style", layout::doc_style),
        ("no-placeholders", layout::no_placeholders),
        ("record-numbers", layout::record_numbers),
        ("dependency-pinning", |root, _| {
            pinning::dependency_pinning(root)
        }),
        ("config-access", source::config_access),
        ("no-suppressions", source::no_suppressions),
        ("version-sync", |root, _| release::version_sync(root, None)),
    ]
}

fn run_checks(
    root: &Path,
    settings: &Settings,
    checks: Vec<(&'static str, Check)>,
) -> Result<Violations, String> {
    let mut violations = Violations::new();
    for (name, check) in checks {
        let found = check(root, settings).map_err(|error| format!("{name}: {error}"))?;
        violations.extend(
            found
                .into_iter()
                .map(|violation| format!("[{name}] {violation}")),
        );
    }
    Ok(violations)
}

fn merged_result(root: &Path, settings: &Settings) -> Result<Violations, String> {
    run_checks(root, settings, whole_repository_checks())
}

fn hook(root: &Path, settings: &Settings) -> Result<Violations, String> {
    let mut violations = source::no_comments(root, settings, &DiffSource::Staged)?
        .into_iter()
        .map(|violation| format!("[no-comments] {violation}"))
        .collect::<Violations>();
    violations.extend(run_checks(root, settings, whole_repository_checks())?);
    Ok(violations)
}

fn run_command(root: &Path, program: &str, args: &[&str]) -> Result<bool, String> {
    println!("$ {program} {}", args.join(" "));
    Command::new(program)
        .args(args)
        .current_dir(root)
        .status()
        .map(|status| status.success())
        .map_err(|error| format!("cannot run {program}: {error}"))
}

fn full(root: &Path) -> Result<Violations, String> {
    let commands: [(&str, &[&str]); 6] = [
        ("cargo", &["fmt", "--all", "--check"]),
        (
            "cargo",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--locked",
                "--",
                "-D",
                "warnings",
            ],
        ),
        ("cargo", &["test", "--workspace", "--locked"]),
        ("npx", &["--no-install", "markdownlint-cli2"]),
        ("npm", &["run", "--silent", "duplicates"]),
        ("cargo", &["xtask", "merged-result"]),
    ];
    let mut violations = Violations::new();
    for (program, args) in commands {
        if !run_command(root, program, args)? {
            violations.push(format!("[full] {program} {} failed.", args.join(" ")));
        }
    }
    Ok(violations)
}

struct WorkspacePackage {
    name: String,
    directory: PathBuf,
    dependencies: BTreeSet<String>,
}

fn workspace_packages(root: &Path) -> Result<Vec<WorkspacePackage>, String> {
    let output = Command::new("cargo")
        .args(["metadata", "--locked", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot run cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("cannot parse cargo metadata: {error}"))?;
    let packages = metadata
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or("cargo metadata lacks packages")?;
    Ok(packages
        .iter()
        .filter_map(|package| {
            let name = package.get("name")?.as_str()?.to_string();
            let manifest = PathBuf::from(package.get("manifest_path")?.as_str()?);
            let directory = manifest.parent()?.to_path_buf();
            let dependencies = package
                .get("dependencies")?
                .as_array()?
                .iter()
                .filter_map(|dependency| dependency.get("name")?.as_str().map(str::to_string))
                .collect();
            Some(WorkspacePackage {
                name,
                directory,
                dependencies,
            })
        })
        .collect())
}

pub(crate) fn affected_packages(
    changed: &[String],
    root: &Path,
    packages: &[(String, PathBuf, BTreeSet<String>)],
) -> BTreeSet<String> {
    let workspace_wide = changed.iter().any(|path| {
        matches!(
            path.as_str(),
            "Cargo.toml" | "Cargo.lock" | "rust-toolchain.toml"
        ) || path.starts_with(".cargo/")
    });
    if workspace_wide {
        return packages.iter().map(|(name, _, _)| name.clone()).collect();
    }
    let mut affected: BTreeSet<String> = changed
        .iter()
        .filter_map(|path| {
            let full = root.join(path);
            packages
                .iter()
                .filter(|(_, directory, _)| full.starts_with(directory))
                .max_by_key(|(_, directory, _)| directory.components().count())
                .map(|(name, _, _)| name.clone())
        })
        .collect();
    loop {
        let dependents: Vec<String> = packages
            .iter()
            .filter(|(name, _, dependencies)| {
                !affected.contains(name)
                    && dependencies
                        .iter()
                        .any(|dependency| affected.contains(dependency))
            })
            .map(|(name, _, _)| name.clone())
            .collect();
        if dependents.is_empty() {
            return affected;
        }
        affected.extend(dependents);
    }
}

fn affected_tests(root: &Path, args: &[String]) -> Result<Violations, String> {
    let base = flag(args, "--base").unwrap_or_else(|_| "origin/main".to_string());
    let merge_base = git::run(root, &["merge-base", &base, "HEAD"])?
        .trim()
        .to_string();
    let mut changed: Vec<String> = git::run(root, &["diff", "--name-only", &merge_base])?
        .lines()
        .map(str::to_string)
        .collect();
    changed.extend(
        git::run(root, &["ls-files", "--others", "--exclude-standard"])?
            .lines()
            .map(str::to_string),
    );
    let packages: Vec<(String, PathBuf, BTreeSet<String>)> = workspace_packages(root)?
        .into_iter()
        .map(|package| (package.name, package.directory, package.dependencies))
        .collect();
    let affected = affected_packages(&changed, root, &packages);
    if affected.is_empty() {
        println!("No affected crates; nothing to test.");
        return Ok(Vec::new());
    }
    let mut args = vec!["test", "--locked"];
    for name in &affected {
        args.push("--package");
        args.push(name);
    }
    Ok(if run_command(root, "cargo", &args)? {
        Vec::new()
    } else {
        vec![format!(
            "[affected-tests] tests failed in {}.",
            affected.iter().cloned().collect::<Vec<_>>().join(", ")
        )]
    })
}

fn feedback_budget(
    root: &Path,
    settings: &Settings,
    args: &[String],
) -> Result<Violations, String> {
    let feedback_loop = FeedbackLoop::from_name(&flag(args, "--loop")?)?;
    let seconds = match flag(args, "--seconds") {
        Ok(seconds) => seconds
            .parse::<f64>()
            .map_err(|_| format!("--seconds must be a number, not {seconds}"))?,
        Err(_) => {
            let jobs = budget::fetch_ci_gate_jobs(root, &flag(args, "--run-id")?)?;
            budget::ci_gate_seconds(&jobs, &flag(args, "--exclude-job")?)
                .ok_or("the run has no completed jobs")?
        }
    };
    let budget_seconds = feedback_loop.budget_seconds(settings);
    println!(
        "{} took {seconds:.0} seconds of a {budget_seconds:.0}-second budget.",
        feedback_loop.title()
    );
    let report = args.iter().any(|arg| arg == "--report");
    Ok(
        match budget::verdict(seconds, budget_seconds, settings.budgets.warn_ratio) {
            Verdict::Within => Vec::new(),
            Verdict::NearBudget => {
                if report {
                    budget::report_near_budget(root, feedback_loop, seconds, budget_seconds)?;
                }
                Vec::new()
            }
            Verdict::OverBudget => vec![budget::over_budget_message(
                feedback_loop,
                seconds,
                budget_seconds,
            )],
        },
    )
}

fn perf_check(root: &Path, settings: &Settings, args: &[String]) -> Result<Violations, String> {
    let binary = flag(args, "--binary").ok().map(PathBuf::from);
    perf::run_perf_check(root, settings, binary.as_deref())
}

fn version_sync(root: &Path, args: &[String]) -> Result<Violations, String> {
    let tag = flag(args, "--tag").ok();
    release::version_sync(root, tag.as_deref())
}

fn release_notes(root: &Path, args: &[String]) -> Result<Violations, String> {
    let since = flag(args, "--since").ok();
    let output = flag(args, "--output").ok();
    let (_, notes) = release::draft_release_notes(root, since.as_deref())?;
    if let Some(path) = output {
        let target = PathBuf::from(&path);
        if let Some(parent) = target.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create parent directory for {path}: {error}"))?;
        }
        fs::write(&target, &notes).map_err(|error| format!("cannot write {path}: {error}"))?;
    } else {
        print!("{notes}");
    }
    Ok(Vec::new())
}

fn prepare_release(root: &Path, args: &[String]) -> Result<Violations, String> {
    let version = flag(args, "--version")?;
    let since = flag(args, "--since").ok();
    let dry_run = args.iter().any(|arg| arg == "--dry-run");
    release::prepare_release(root, &version, since.as_deref(), dry_run)
}

#[cfg(test)]
mod tests;
