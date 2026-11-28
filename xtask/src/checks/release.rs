use std::fs;
use std::path::Path;
use std::process::Command;

use toml::{Table, Value};

use super::Violations;
use super::pinning::is_exact_version;
use crate::git;

pub(crate) const PLATFORM_PACKAGES: [&str; 6] = [
    "darwin-arm64",
    "darwin-x64",
    "linux-arm64",
    "linux-x64",
    "win32-arm64",
    "win32-x64",
];

const WRAPPER_MANIFEST: &str = "npm/codeint/package.json";
const WORKSPACE_MANIFEST: &str = "Cargo.toml";
const DART_PUBSPEC_MANIFEST: &str = "sdks/dart/pubspec.yaml";
const DART_OPTIONS_SOURCE: &str = "sdks/dart/lib/src/options.dart";
const DART_VERSION_PREFIX: &str = "const String codeintVersion = ";

fn platform_manifest_path(platform: &str) -> String {
    format!("npm/platforms/{platform}/package.json")
}

fn platform_package_name(platform: &str) -> String {
    format!("@avidian/codeint-{platform}")
}

fn replace_first_line(
    text: &str,
    missing_error: impl FnOnce() -> String,
    mut rewrite: impl FnMut(&str) -> Option<String>,
) -> Result<String, String> {
    let mut replaced = false;
    let mut lines = Vec::new();
    for line in text.lines() {
        if !replaced && let Some(next) = rewrite(line) {
            lines.push(next);
            replaced = true;
            continue;
        }
        lines.push(line.to_string());
    }
    if !replaced {
        return Err(missing_error());
    }
    let mut updated = lines.join("\n");
    if text.ends_with('\n') {
        updated.push('\n');
    }
    Ok(updated)
}

fn discover_pubspec_manifests(root: &Path) -> Result<Vec<String>, String> {
    let mut manifests = Vec::new();
    let sdks_dir = root.join("sdks");
    if sdks_dir.is_dir() {
        let entries = fs::read_dir(&sdks_dir)
            .map_err(|error| format!("cannot read sdks directory: {error}"))?;
        for entry in entries {
            let entry = entry.map_err(|error| format!("cannot read sdks entry: {error}"))?;
            if !entry.path().is_dir() {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let relative = format!("sdks/{name}/pubspec.yaml");
            if root.join(&relative).is_file() {
                manifests.push(relative);
            }
        }
    }
    if !manifests.iter().any(|path| path == DART_PUBSPEC_MANIFEST) {
        manifests.push(DART_PUBSPEC_MANIFEST.to_string());
    }
    manifests.sort();
    Ok(manifests)
}

pub(crate) fn pubspec_version(text: &str) -> Option<&str> {
    text.lines().find_map(|line| {
        let rest = line.strip_prefix("version:")?;
        let value = rest.trim().trim_matches('\'').trim_matches('"');
        (!value.is_empty()).then_some(value)
    })
}

pub(crate) fn replace_pubspec_version(text: &str, new_version: &str) -> Result<String, String> {
    replace_first_line(
        text,
        || "pubspec.yaml lacks a top-level \"version:\" line".to_string(),
        |line| {
            line.starts_with("version:")
                .then(|| format!("version: {new_version}"))
        },
    )
}

pub(crate) fn dart_codeint_version(text: &str) -> Option<&str> {
    text.lines().find_map(|line| {
        let rest = line
            .trim()
            .strip_prefix(DART_VERSION_PREFIX)?
            .strip_suffix(';')?
            .trim();
        let value = rest
            .strip_prefix('\'')
            .and_then(|inner| inner.strip_suffix('\''))
            .or_else(|| {
                rest.strip_prefix('"')
                    .and_then(|inner| inner.strip_suffix('"'))
            })?;
        (!value.is_empty()).then_some(value)
    })
}

pub(crate) fn replace_dart_codeint_version(
    text: &str,
    new_version: &str,
) -> Result<String, String> {
    replace_first_line(
        text,
        || format!("{DART_OPTIONS_SOURCE} lacks a const String codeintVersion declaration"),
        |line| {
            line.trim_start().starts_with(DART_VERSION_PREFIX).then(|| {
                let indent: String = line
                    .chars()
                    .take_while(|character| character.is_whitespace())
                    .collect();
                format!("{indent}{DART_VERSION_PREFIX}'{new_version}';")
            })
        },
    )
}

pub(crate) fn workspace_version(root: &Path) -> Result<String, String> {
    let text = fs::read_to_string(root.join(WORKSPACE_MANIFEST))
        .map_err(|error| format!("cannot read {WORKSPACE_MANIFEST}: {error}"))?;
    let table: Table = text
        .parse()
        .map_err(|error| format!("{WORKSPACE_MANIFEST} is not valid TOML: {error}"))?;
    let version = table
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("version"))
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{WORKSPACE_MANIFEST} lacks [workspace.package].version"))?;
    if !is_exact_version(version) {
        return Err(format!(
            "{WORKSPACE_MANIFEST} [workspace.package].version \"{version}\" is not an exact SemVer version"
        ));
    }
    Ok(version.to_string())
}

fn check_release_tag(tag: Option<&str>, expected: &str, violations: &mut Violations) {
    if let Some(tag) = tag {
        let expected_tag = format!("v{expected}");
        if tag != expected_tag {
            violations.push(format!(
                "Release tag \"{tag}\" disagrees with manifest version \"{expected}\" (expected \"{expected_tag}\")."
            ));
        }
    }
}

fn check_wrapper_optional_deps(
    wrapper: &serde_json::Value,
    expected: &str,
    violations: &mut Violations,
) {
    let Some(deps) = wrapper
        .get("optionalDependencies")
        .and_then(serde_json::Value::as_object)
    else {
        violations.push(format!(
            "{WRAPPER_MANIFEST} lacks an \"optionalDependencies\" object."
        ));
        return;
    };

    for platform in PLATFORM_PACKAGES {
        let package_name = platform_package_name(platform);
        match deps.get(&package_name).and_then(serde_json::Value::as_str) {
            Some(actual) if actual == expected => {}
            Some(actual) => violations.push(format!(
                "{WRAPPER_MANIFEST} optionalDependencies.{package_name} is \"{actual}\", expected \"{expected}\"."
            )),
            None => violations.push(format!(
                "{WRAPPER_MANIFEST} optionalDependencies lacks \"{package_name}\"."
            )),
        }
    }

    for (name, value) in deps {
        let is_known = PLATFORM_PACKAGES
            .iter()
            .any(|platform| platform_package_name(platform) == *name);
        if !is_known {
            violations.push(format!(
                "{WRAPPER_MANIFEST} optionalDependencies has unexpected package \"{name}\"."
            ));
        } else if value.as_str().is_none() {
            violations.push(format!(
                "{WRAPPER_MANIFEST} optionalDependencies.{name} must be a version string."
            ));
        }
    }
}

fn rewrite_file(
    root: &Path,
    relative: &str,
    transform: impl FnOnce(&str) -> Result<String, String>,
) -> Result<(), String> {
    let path = root.join(relative);
    let text =
        fs::read_to_string(&path).map_err(|error| format!("cannot read {relative}: {error}"))?;
    let updated = transform(&text)?;
    fs::write(&path, updated).map_err(|error| format!("cannot write {relative}: {error}"))
}

fn check_version_field(
    path: &str,
    field: &str,
    missing_message: &str,
    actual: Option<&str>,
    expected: &str,
    violations: &mut Violations,
) {
    match actual {
        Some(value) if value == expected => {}
        Some(value) => violations.push(format!(
            "{path} {field} is \"{value}\", expected \"{expected}\" from {WORKSPACE_MANIFEST}."
        )),
        None => violations.push(format!("{path} {missing_message}")),
    }
}

fn check_wrapper_manifest(
    root: &Path,
    expected: &str,
    violations: &mut Violations,
) -> Result<(), String> {
    let wrapper_text = fs::read_to_string(root.join(WRAPPER_MANIFEST))
        .map_err(|error| format!("cannot read {WRAPPER_MANIFEST}: {error}"))?;
    let wrapper: serde_json::Value = serde_json::from_str(&wrapper_text)
        .map_err(|error| format!("{WRAPPER_MANIFEST} is not valid JSON: {error}"))?;
    check_version_field(
        WRAPPER_MANIFEST,
        "version",
        "lacks a string \"version\" field.",
        wrapper.get("version").and_then(serde_json::Value::as_str),
        expected,
        violations,
    );
    check_wrapper_optional_deps(&wrapper, expected, violations);
    Ok(())
}

fn check_platform_manifests(
    root: &Path,
    expected: &str,
    violations: &mut Violations,
) -> Result<(), String> {
    for platform in PLATFORM_PACKAGES {
        let path = platform_manifest_path(platform);
        let text = fs::read_to_string(root.join(&path))
            .map_err(|error| format!("cannot read {path}: {error}"))?;
        let manifest: serde_json::Value = serde_json::from_str(&text)
            .map_err(|error| format!("{path} is not valid JSON: {error}"))?;
        check_version_field(
            &path,
            "version",
            "lacks a string \"version\" field.",
            manifest.get("version").and_then(serde_json::Value::as_str),
            expected,
            violations,
        );
    }
    Ok(())
}

fn check_pubspec_manifests(
    root: &Path,
    expected: &str,
    violations: &mut Violations,
) -> Result<(), String> {
    for path in discover_pubspec_manifests(root)? {
        let text = fs::read_to_string(root.join(&path))
            .map_err(|error| format!("cannot read {path}: {error}"))?;
        check_version_field(
            &path,
            "version",
            "lacks a top-level \"version\" field.",
            pubspec_version(&text),
            expected,
            violations,
        );
    }
    Ok(())
}

fn check_dart_options_version(
    root: &Path,
    expected: &str,
    violations: &mut Violations,
) -> Result<(), String> {
    let options_text = fs::read_to_string(root.join(DART_OPTIONS_SOURCE))
        .map_err(|error| format!("cannot read {DART_OPTIONS_SOURCE}: {error}"))?;
    check_version_field(
        DART_OPTIONS_SOURCE,
        "codeintVersion",
        "lacks a const String codeintVersion declaration.",
        dart_codeint_version(&options_text),
        expected,
        violations,
    );
    Ok(())
}

pub(crate) fn version_sync(root: &Path, tag: Option<&str>) -> Result<Violations, String> {
    let expected = workspace_version(root)?;
    let mut violations = Violations::new();
    check_release_tag(tag, &expected, &mut violations);
    check_wrapper_manifest(root, &expected, &mut violations)?;
    check_platform_manifests(root, &expected, &mut violations)?;
    check_pubspec_manifests(root, &expected, &mut violations)?;
    check_dart_options_version(root, &expected, &mut violations)?;
    Ok(violations)
}

#[derive(Debug, PartialEq, Eq)]
struct ParsedSubject {
    kind: String,
    scope: Option<String>,
    breaking: bool,
    summary: String,
}

fn parse_conventional_subject(subject: &str) -> Option<ParsedSubject> {
    let (header, summary) = subject.split_once(": ")?;
    let summary = summary.trim();
    if summary.is_empty() {
        return None;
    }
    let (header, breaking) = match header.strip_suffix('!') {
        Some(stripped) => (stripped, true),
        None => (header, false),
    };
    let (kind, scope) = match header.split_once('(') {
        Some((kind, rest)) => (kind, Some(rest.strip_suffix(')')?.to_string())),
        None => (header, None),
    };
    if kind.is_empty() || !kind.chars().all(|character| character.is_ascii_lowercase()) {
        return None;
    }
    Some(ParsedSubject {
        kind: kind.to_string(),
        scope,
        breaking,
        summary: summary.to_string(),
    })
}

fn format_entry(scope: Option<&str>, summary: &str) -> String {
    match scope {
        Some(scope) if !scope.is_empty() => format!("- **{scope}:** {summary}"),
        _ => format!("- {summary}"),
    }
}

pub(crate) fn format_release_notes(subjects: &[String], since: Option<&str>) -> String {
    let mut breaking = Vec::new();
    let mut features = Vec::new();
    let mut fixes = Vec::new();
    let mut performance = Vec::new();
    let mut documentation = Vec::new();
    let mut other = Vec::new();

    for raw in subjects {
        let subject = raw.trim();
        if subject.is_empty() {
            continue;
        }
        match parse_conventional_subject(subject) {
            Some(parsed) => {
                let line = format_entry(parsed.scope.as_deref(), &parsed.summary);
                if parsed.breaking {
                    breaking.push(line);
                } else {
                    match parsed.kind.as_str() {
                        "feat" => features.push(line),
                        "fix" => fixes.push(line),
                        "perf" => performance.push(line),
                        "docs" => documentation.push(line),
                        _ => other.push(line),
                    }
                }
            }
            None => other.push(format!("- {subject}")),
        }
    }

    let sections: [(&str, &[String]); 6] = [
        ("Breaking Changes", &breaking),
        ("Features", &features),
        ("Fixes", &fixes),
        ("Performance", &performance),
        ("Documentation", &documentation),
        ("Other Changes", &other),
    ];

    let mut rendered = Vec::new();
    for (title, items) in sections {
        if !items.is_empty() {
            let mut block = format!("### {title}\n\n");
            block.push_str(&items.join("\n"));
            rendered.push(block);
        }
    }

    if rendered.is_empty() {
        return match since {
            Some(reference) => format!("No changes since {reference}.\n"),
            None => "No changes.\n".to_string(),
        };
    }

    let mut output = rendered.join("\n\n");
    output.push('\n');
    output
}

pub(crate) fn previous_release_tag(root: &Path) -> Option<String> {
    let output = git::run(
        root,
        &["describe", "--tags", "--abbrev=0", "--match", "v*", "HEAD^"],
    )
    .ok()?;
    let tag = output.trim();
    if tag.is_empty() {
        None
    } else {
        Some(tag.to_string())
    }
}

pub(crate) fn draft_release_notes(
    root: &Path,
    since: Option<&str>,
) -> Result<(Option<String>, String), String> {
    let resolved_since = match since {
        Some(reference) => Some(reference.to_string()),
        None => previous_release_tag(root),
    };
    let range = match &resolved_since {
        Some(reference) => format!("{reference}..HEAD"),
        None => "HEAD".to_string(),
    };
    let log = git::run(root, &["log", "--no-merges", "--format=%s", &range])?;
    let subjects: Vec<String> = log.lines().map(str::to_string).collect();
    let notes = format_release_notes(&subjects, resolved_since.as_deref());
    Ok((resolved_since, notes))
}

pub(crate) fn replace_workspace_package_version(
    text: &str,
    new_version: &str,
) -> Result<String, String> {
    let mut in_workspace_package = false;
    replace_first_line(
        text,
        || format!("{WORKSPACE_MANIFEST} lacks a version line under [workspace.package]"),
        |line| {
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                in_workspace_package = trimmed == "[workspace.package]";
                return None;
            }
            if in_workspace_package
                && trimmed.starts_with("version")
                && let Some((key, _)) = trimmed.split_once('=')
                && key.trim() == "version"
            {
                let indent: String = line
                    .chars()
                    .take_while(|character| character.is_whitespace())
                    .collect();
                return Some(format!("{indent}version = \"{new_version}\""));
            }
            None
        },
    )
}

pub(crate) fn replace_json_string_field(
    text: &str,
    key: &str,
    new_value: &str,
) -> Result<String, String> {
    let needle = format!("\"{key}\": \"");
    replace_first_line(
        text,
        || format!("JSON manifest lacks string field \"{key}\""),
        |line| {
            let prefix_end = line.find(&needle)?;
            let value_start = prefix_end + needle.len();
            let value_len = line[value_start..].find('"')?;
            Some(format!(
                "{}{new_value}{}",
                &line[..value_start],
                &line[value_start + value_len..]
            ))
        },
    )
}

fn update_wrapper_manifest(root: &Path, new_version: &str) -> Result<(), String> {
    rewrite_file(root, WRAPPER_MANIFEST, |text| {
        let mut updated = replace_json_string_field(text, "version", new_version)?;
        for platform in PLATFORM_PACKAGES {
            let package_name = platform_package_name(platform);
            updated = replace_json_string_field(&updated, &package_name, new_version)?;
        }
        serde_json::from_str::<serde_json::Value>(&updated)
            .map_err(|error| format!("updated {WRAPPER_MANIFEST} is not valid JSON: {error}"))?;
        Ok(updated)
    })
}

fn update_platform_manifests(root: &Path, new_version: &str) -> Result<(), String> {
    for platform in PLATFORM_PACKAGES {
        let relative = platform_manifest_path(platform);
        rewrite_file(root, &relative, |text| {
            let updated = replace_json_string_field(text, "version", new_version)?;
            serde_json::from_str::<serde_json::Value>(&updated)
                .map_err(|error| format!("updated {relative} is not valid JSON: {error}"))?;
            Ok(updated)
        })?;
    }
    Ok(())
}

fn update_sdk_versions(root: &Path, new_version: &str) -> Result<(), String> {
    for relative in discover_pubspec_manifests(root)? {
        rewrite_file(root, &relative, |text| {
            replace_pubspec_version(text, new_version)
        })?;
    }
    rewrite_file(root, DART_OPTIONS_SOURCE, |text| {
        replace_dart_codeint_version(text, new_version)
    })
}

fn apply_release_updates(root: &Path, new_version: &str) -> Result<(), String> {
    rewrite_file(root, WORKSPACE_MANIFEST, |text| {
        replace_workspace_package_version(text, new_version)
    })?;
    update_wrapper_manifest(root, new_version)?;
    update_platform_manifests(root, new_version)?;
    update_sdk_versions(root, new_version)?;
    refresh_cargo_lock(root)
}

fn refresh_cargo_lock(root: &Path) -> Result<(), String> {
    if !root.join("Cargo.lock").exists() {
        return Ok(());
    }
    let output = Command::new("cargo")
        .args(["update", "--workspace", "--offline"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot run cargo update --workspace --offline: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo update --workspace --offline failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

fn print_release_notes(resolved_since: Option<&str>, notes: &str) {
    if let Some(reference) = resolved_since {
        println!("Release notes since {reference}:\n");
    } else {
        println!("Release notes:\n");
    }
    print!("{notes}");
}

pub(crate) fn prepare_release(
    root: &Path,
    new_version: &str,
    since: Option<&str>,
    dry_run: bool,
) -> Result<Violations, String> {
    if !is_exact_version(new_version) {
        return Err(format!(
            "release version \"{new_version}\" is not an exact SemVer version (expected X.Y.Z)"
        ));
    }
    let current = workspace_version(root)?;
    let (resolved_since, notes) = draft_release_notes(root, since)?;

    if !dry_run {
        apply_release_updates(root, new_version)?;
    }

    let violations = version_sync(root, None)?;
    if !violations.is_empty() {
        return Ok(violations);
    }

    if dry_run {
        println!("Dry run: would update version {current} -> {new_version} (no files modified).");
    } else {
        println!("Updated manifests from {current} to {new_version}.");
    }
    print_release_notes(resolved_since.as_deref(), &notes);
    Ok(Vec::new())
}

#[cfg(test)]
mod tests;
