use std::fs;
use std::path::Path;
use std::process::Command;

use toml::{Table, Value};

use super::Violations;
use crate::git;

const CARGO_SECTIONS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];
const NPM_SECTIONS: [&str; 3] = ["dependencies", "devDependencies", "optionalDependencies"];

pub(crate) fn is_exact_cargo_requirement(requirement: &str) -> bool {
    requirement
        .strip_prefix('=')
        .is_some_and(|version| is_exact_version(version.trim()))
}

pub(crate) fn is_exact_version(version: &str) -> bool {
    let (rest, build) = match version.split_once('+') {
        Some((rest, build)) => (rest, Some(build)),
        None => (version, None),
    };
    let (core, prerelease) = match rest.split_once('-') {
        Some((core, prerelease)) => (core, Some(prerelease)),
        None => (rest, None),
    };
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|part| is_numeric_identifier(part))
        && prerelease.is_none_or(is_dotted_identifier)
        && build.is_none_or(is_dotted_identifier)
}

fn is_numeric_identifier(part: &str) -> bool {
    !part.is_empty() && part.chars().all(|character| character.is_ascii_digit())
}

fn is_dotted_identifier(text: &str) -> bool {
    text.split('.').all(|identifier| {
        !identifier.is_empty()
            && identifier
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-')
    })
}

fn cargo_dependency_problem(spec: &Value) -> Option<String> {
    match spec {
        Value::String(requirement) if is_exact_cargo_requirement(requirement) => None,
        Value::String(requirement) => Some(format!("asks for \"{requirement}\"")),
        Value::Table(table) => {
            if table.get("workspace").and_then(Value::as_bool) == Some(true) {
                return None;
            }
            if table.contains_key("git") {
                return Some("comes from Git".to_string());
            }
            match table.get("version").and_then(Value::as_str) {
                Some(requirement) if is_exact_cargo_requirement(requirement) => None,
                Some(requirement) => Some(format!("asks for \"{requirement}\"")),
                None if table.contains_key("path") => None,
                None => Some("has no version".to_string()),
            }
        }
        _ => Some("has an unreadable version".to_string()),
    }
}

fn dependency_tables(manifest: &Table) -> Vec<&Table> {
    let mut tables = Vec::new();
    for section in CARGO_SECTIONS {
        if let Some(table) = manifest.get(section).and_then(Value::as_table) {
            tables.push(table);
        }
    }
    if let Some(table) = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(Value::as_table)
    {
        tables.push(table);
    }
    if let Some(targets) = manifest.get("target").and_then(Value::as_table) {
        for target in targets.values() {
            for section in CARGO_SECTIONS {
                if let Some(table) = target.get(section).and_then(Value::as_table) {
                    tables.push(table);
                }
            }
        }
    }
    tables
}

pub(crate) fn cargo_manifest_problems(path: &str, text: &str) -> Result<Violations, String> {
    let manifest: Table = text
        .parse()
        .map_err(|error| format!("{path} is not valid TOML: {error}"))?;
    let mut violations = Violations::new();
    for table in dependency_tables(&manifest) {
        for (name, spec) in table {
            if let Some(problem) = cargo_dependency_problem(spec) {
                violations.push(format!(
                    "{name} in {path} is not pinned to an exact version: it {problem}. Pin it with \"=x.y.z\" and update Cargo.lock."
                ));
            }
        }
    }
    Ok(violations)
}

pub(crate) fn npm_manifest_problems(path: &str, text: &str) -> Result<Violations, String> {
    let manifest: serde_json::Value =
        serde_json::from_str(text).map_err(|error| format!("{path} is not valid JSON: {error}"))?;
    let mut violations = Violations::new();
    for section in NPM_SECTIONS {
        let Some(dependencies) = manifest.get(section).and_then(serde_json::Value::as_object)
        else {
            continue;
        };
        for (name, version) in dependencies {
            let exact = version.as_str().is_some_and(is_exact_version);
            if !exact {
                violations.push(format!(
                    "{name} in {path} is not pinned to an exact version. Pin it to x.y.z and update package-lock.json."
                ));
            }
        }
    }
    Ok(violations)
}

fn npm_lock_problems(
    root: &Path,
    manifest_path: &str,
    manifest_text: &str,
) -> Result<Violations, String> {
    let lock_path = manifest_path.replace("package.json", "package-lock.json");
    let Ok(lock_text) = fs::read_to_string(root.join(&lock_path)) else {
        return Ok(vec![format!(
            "{manifest_path} has no committed {lock_path}. Run npm install and commit the lockfile."
        )]);
    };
    let manifest: serde_json::Value = serde_json::from_str(manifest_text)
        .map_err(|error| format!("{manifest_path} is not valid JSON: {error}"))?;
    let lock: serde_json::Value = serde_json::from_str(&lock_text)
        .map_err(|error| format!("{lock_path} is not valid JSON: {error}"))?;
    let locked_root = lock.get("packages").and_then(|packages| packages.get(""));
    let stale = NPM_SECTIONS.iter().any(|section| {
        let declared = manifest
            .get(*section)
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let locked = locked_root
            .and_then(|root| root.get(*section))
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        declared != locked
    });
    Ok(if stale {
        vec![format!(
            "{lock_path} is stale for {manifest_path}. Run npm install and commit the lockfile."
        )]
    } else {
        Vec::new()
    })
}

fn cargo_lock_is_current(root: &Path) -> Result<bool, String> {
    let status = Command::new("cargo")
        .args(["metadata", "--locked", "--format-version", "1", "--quiet"])
        .current_dir(root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|error| format!("cannot run cargo metadata: {error}"))?;
    Ok(status.success())
}

pub(crate) fn dependency_pinning(root: &Path) -> Result<Violations, String> {
    let mut violations = Violations::new();
    let mut has_cargo_manifest = false;
    for path in git::tracked_files(root)? {
        let name = path.rsplit('/').next().unwrap_or(&path);
        let text = || {
            fs::read_to_string(root.join(&path))
                .map_err(|error| format!("cannot read {path}: {error}"))
        };
        if name == "Cargo.toml" {
            has_cargo_manifest = true;
            violations.extend(cargo_manifest_problems(&path, &text()?)?);
        } else if name == "package.json" {
            let manifest_text = text()?;
            violations.extend(npm_manifest_problems(&path, &manifest_text)?);
            violations.extend(npm_lock_problems(root, &path, &manifest_text)?);
        }
    }
    if has_cargo_manifest && !cargo_lock_is_current(root)? {
        violations.push(
            "Cargo.lock is missing or stale. Run cargo update --workspace --offline or cargo build, and commit Cargo.lock.".to_string(),
        );
    }
    Ok(violations)
}

#[cfg(test)]
mod tests;
