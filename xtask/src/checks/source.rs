use std::fs;
use std::path::Path;

use super::{Violations, glob_set};
use crate::config::Settings;
use crate::git::{self, DiffSource};
use crate::lexer::{self, Language};

const HASH_DIRECTIVES: [&str; 2] = ["# yaml-language-server:", "# shellcheck shell="];

const DIRECTIVE_SUPPRESSIONS: [&str; 11] = [
    "eslint-disable",
    "@ts-ignore",
    "@ts-expect-error",
    "@ts-nocheck",
    "biome-ignore",
    "oxlint-disable",
    "shellcheck disable",
    "yamllint disable",
    "gitleaks:allow",
    "noqa",
    "nosec",
];

const MARKDOWN_SUPPRESSIONS: [&str; 2] = ["markdownlint-disable", "markdownlint-configure-file"];

const CONFIG_READS: [&str; 5] = ["env::var", "env::args", "env::{", "env::*", "env as "];

const SHEBANG_LANGUAGES: [Language; 3] = [Language::Shell, Language::PowerShell, Language::Script];

pub(crate) fn comment_is_allowed(
    language: Language,
    line_number: usize,
    raw_line: &str,
    doc: bool,
) -> bool {
    let trimmed = raw_line.trim_start();
    let shebang =
        line_number == 1 && SHEBANG_LANGUAGES.contains(&language) && raw_line.starts_with("#!/");
    let wit_doc = language == Language::Wit && doc;
    let directive = HASH_DIRECTIVES
        .iter()
        .any(|directive| trimmed.starts_with(directive));
    shebang || wit_doc || directive
}

pub(crate) fn comment_lines_added(language: Language, text: &str, added: &[usize]) -> Vec<usize> {
    let scan = lexer::scan(language, text);
    let raw_lines: Vec<&str> = text.lines().collect();
    added
        .iter()
        .copied()
        .filter(|line| scan.is_comment(*line))
        .filter(|line| {
            let raw = line
                .checked_sub(1)
                .and_then(|index| raw_lines.get(index))
                .copied()
                .unwrap_or("");
            !comment_is_allowed(language, *line, raw, scan.is_doc_comment(*line))
        })
        .collect()
}

pub(crate) fn no_comments(
    root: &Path,
    settings: &Settings,
    source: &DiffSource,
) -> Result<Violations, String> {
    let mut violations = Violations::new();
    let test_input = super::glob_set(&settings.test_input_paths)?;
    for change in git::changed_files(root, source)? {
        if change.is_deleted()
            || settings.is_third_party_skill(&change.path)
            || test_input.is_match(&change.path)
        {
            continue;
        }
        let Some(language) = Language::of_path(&change.path) else {
            continue;
        };
        let diff = git::line_diff(root, source, &change.path)?;
        let added: Vec<usize> = diff.added.iter().map(|(line, _)| *line).collect();
        let text = git::new_content(root, source, &change.path)?;
        for line in comment_lines_added(language, &text, &added) {
            violations.push(format!(
                "{}:{line} adds a comment. Delete it. Put the reason in a name, an ADR, a test name, the commit message, or a tracker issue; see docs/agents/engineering.md.",
                change.path
            ));
        }
    }
    Ok(violations)
}

pub(crate) fn suppressions_in(path: &str, text: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    if path.ends_with(".md") || path == ".rules" {
        for (index, line) in text.lines().enumerate() {
            if markdown_suppression(line) {
                found.push((index + 1, "markdownlint".to_string()));
            }
        }
        return found;
    }
    let Some(language) = Language::of_path(path) else {
        return found;
    };
    if language == Language::Rust {
        let scan = lexer::scan(language, text);
        for (index, code) in scan.code_lines.iter().enumerate() {
            if let Some(rule) = rust_attribute_suppression(code) {
                found.push((index + 1, rule));
            }
        }
        return found;
    }
    for (index, line) in text.lines().enumerate() {
        if let Some(directive) = DIRECTIVE_SUPPRESSIONS
            .iter()
            .find(|directive| line.contains(**directive))
        {
            found.push((index + 1, (*directive).to_string()));
        }
    }
    found
}

fn markdown_suppression(line: &str) -> bool {
    let lowered = line.to_lowercase();
    lowered.match_indices("<!--").any(|(position, _)| {
        let directive = lowered[position + 4..].trim_start();
        MARKDOWN_SUPPRESSIONS
            .iter()
            .any(|suppression| directive.starts_with(suppression))
    })
}

fn read_text(path: &Path) -> Result<Option<String>, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    if bytes.contains(&0) {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
}

fn rust_attribute_suppression(code: &str) -> Option<String> {
    let attribute_start = code.find("#[").or_else(|| code.find("#!["))?;
    let attribute = &code[attribute_start..];
    ["allow(", "expect("]
        .iter()
        .find_map(|keyword| attribute.find(keyword).map(|position| (keyword, position)))
        .map(|(keyword, position)| {
            let inner = &attribute[position + keyword.len()..];
            let rule = inner.split(')').next().unwrap_or("").trim();
            format!("{}{rule})", keyword)
        })
}

pub(crate) fn no_suppressions(root: &Path, settings: &Settings) -> Result<Violations, String> {
    let approved = glob_set(&settings.approved_suppressions)?;
    let mut violations = Violations::new();
    for path in git::tracked_files(root)? {
        if approved.is_match(&path) || settings.is_third_party_skill(&path) {
            continue;
        }
        let full = root.join(&path);
        if !fs::symlink_metadata(&full).is_ok_and(|metadata| metadata.is_file()) {
            continue;
        }
        let Some(text) = read_text(&full)? else {
            continue;
        };
        for (line, rule) in suppressions_in(&path, &text) {
            violations.push(format!(
                "{path}:{line} suppresses {rule}. Fix the cause, or change the rule for the whole Project through an ADR."
            ));
        }
    }
    Ok(violations)
}

pub(crate) fn config_reads_in(text: &str) -> Vec<usize> {
    let scan = lexer::scan(Language::Rust, text);
    scan.code_lines
        .iter()
        .enumerate()
        .filter(|(_, code)| {
            CONFIG_READS.iter().any(|needle| {
                code.match_indices(needle).any(|(position, _)| {
                    let before = code[..position].chars().last();
                    !before.is_some_and(|character| character.is_alphanumeric() || character == '_')
                })
            })
        })
        .map(|(index, _)| index + 1)
        .collect()
}

pub(crate) fn config_access(root: &Path, settings: &Settings) -> Result<Violations, String> {
    let modules = glob_set(&settings.config_modules)?;
    let mut violations = Violations::new();
    for path in git::tracked_files(root)? {
        if !path.ends_with(".rs") || modules.is_match(&path) {
            continue;
        }
        let full = root.join(&path);
        if !fs::symlink_metadata(&full).is_ok_and(|metadata| metadata.is_file()) {
            continue;
        }
        let Some(text) = read_text(&full)? else {
            continue;
        };
        for line in config_reads_in(&text) {
            violations.push(format!(
                "{path}:{line} reads configuration directly. Read it in a configuration module ({}) and pass the typed value.",
                settings.config_modules.join(", ")
            ));
        }
    }
    Ok(violations)
}

#[cfg(test)]
mod tests;
