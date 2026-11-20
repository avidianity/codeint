use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use globset::GlobSet;
use serde_json::Value;

use super::Violations;
use crate::config::{DuplicatePass, Settings};

const JSCPD: &str = "node_modules/.bin/jscpd";
const REPORT_FILE: &str = "jscpd-report.json";

pub(crate) fn no_duplicate_code(root: &Path, settings: &Settings) -> Result<Violations, String> {
    let test_input = super::glob_set(&settings.test_input_paths)?;
    let mut clones = BTreeSet::new();
    for pass in &settings.duplicate_passes {
        let report = run_jscpd(root, pass)?;
        clones.extend(reportable_clones(&report, &test_input)?);
    }
    Ok(clones.into_iter().collect())
}

fn run_jscpd(root: &Path, pass: &DuplicatePass) -> Result<String, String> {
    let output_folder = report_folder(pass);
    let _ = fs::remove_dir_all(&output_folder);
    let output = Command::new(root.join(JSCPD))
        .args([
            "--config",
            ".jscpd.json",
            "--min-lines",
            &pass.min_lines.to_string(),
            "--min-tokens",
            &pass.min_tokens.to_string(),
            "--reporters",
            "json",
            "--exit-code",
            "0",
            "--output",
        ])
        .arg(&output_folder)
        .arg(".")
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot run {JSCPD}: {error}. Run npm ci first."))?;
    if !output.status.success() {
        return Err(format!(
            "jscpd failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let report_path = output_folder.join(REPORT_FILE);
    let report = fs::read_to_string(&report_path).map_err(|error| {
        format!(
            "jscpd wrote no report at {} ({error}); its output was: {}",
            report_path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )
    })?;
    let _ = fs::remove_dir_all(&output_folder);
    Ok(report)
}

fn report_folder(pass: &DuplicatePass) -> PathBuf {
    std::env::temp_dir().join(format!(
        "xtask-jscpd-{}-{}-{}",
        std::process::id(),
        pass.min_lines,
        pass.min_tokens
    ))
}

pub(crate) fn reportable_clones(report: &str, test_input: &GlobSet) -> Result<Vec<String>, String> {
    let report: Value = serde_json::from_str(report)
        .map_err(|error| format!("the jscpd report is not valid JSON: {error}"))?;
    let duplicates = report
        .get("duplicates")
        .and_then(Value::as_array)
        .ok_or("the jscpd report has no duplicates list")?;
    let mut clones = Vec::new();
    for duplicate in duplicates {
        let first = location(duplicate, "firstFile")?;
        let second = location(duplicate, "secondFile")?;
        if test_input.is_match(&first.0) && test_input.is_match(&second.0) {
            continue;
        }
        clones.push(format!(
            "{}:{}-{} duplicates {}:{}-{}. Extract the shared code into one place; see docs/checks/no-duplicate-code.md.",
            first.0, first.1, first.2, second.0, second.1, second.2
        ));
    }
    Ok(clones)
}

fn location(duplicate: &Value, side: &str) -> Result<(String, u64, u64), String> {
    let file = duplicate
        .get(side)
        .ok_or_else(|| format!("a jscpd clone has no {side}"))?;
    let name = file
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("a jscpd clone has no {side}.name"))?;
    let line = |key: &str| {
        file.get(key)
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("a jscpd clone has no {side}.{key}"))
    };
    Ok((
        name.trim_start_matches("./").to_string(),
        line("start")?,
        line("end")?,
    ))
}

#[cfg(test)]
mod tests;
