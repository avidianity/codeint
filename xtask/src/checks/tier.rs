use std::collections::BTreeSet;
use std::path::Path;

use globset::GlobSet;
use toml::{Table, Value};

use super::glob_set;
use super::source::suppressions_in;
use crate::config::Settings;
use crate::git::{self, DiffSource, FileChange, LineDiff};
use crate::lexer::{self, Language};

pub(crate) const ORACLE_TIER: u8 = 3;

pub(crate) struct TierFinding {
    pub(crate) tier: u8,
    pub(crate) path: String,
    pub(crate) reason: String,
}

pub(crate) struct TierMap {
    rules: Vec<(u8, Vec<String>, GlobSet)>,
    default: u8,
}

impl TierMap {
    pub(crate) fn from_settings(settings: &Settings) -> Result<TierMap, String> {
        let rules = settings
            .tier_rules
            .iter()
            .map(|rule| Ok((rule.tier, rule.paths.clone(), glob_set(&rule.paths)?)))
            .collect::<Result<Vec<_>, String>>()?;
        Ok(TierMap {
            rules,
            default: settings.tier_default,
        })
    }

    pub(crate) fn classify(&self, path: &str) -> TierFinding {
        let mut best: Option<TierFinding> = None;
        for (tier, patterns, set) in &self.rules {
            let matched = set.matches(path);
            let Some(first) = matched.first() else {
                continue;
            };
            if best.as_ref().is_none_or(|current| *tier > current.tier) {
                best = Some(TierFinding {
                    tier: *tier,
                    path: path.to_string(),
                    reason: format!("matches tier map rule \"{}\"", patterns[*first]),
                });
            }
        }
        best.unwrap_or(TierFinding {
            tier: self.default,
            path: path.to_string(),
            reason: "matches no tier map rule, so the default applies".to_string(),
        })
    }
}

fn lint_tables(text: &str) -> Option<(Option<Value>, Option<Value>)> {
    let manifest: Table = text.parse().ok()?;
    let package_lints = manifest.get("lints").cloned();
    let workspace_lints = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("lints"))
        .cloned();
    Some((package_lints, workspace_lints))
}

fn only_inherits_workspace_lints(lints: &(Option<Value>, Option<Value>)) -> bool {
    let mut inherit = Table::new();
    inherit.insert("workspace".to_string(), Value::Boolean(true));
    match lints {
        (None, None) => true,
        (Some(package), None) => *package == Value::Table(inherit),
        _ => false,
    }
}

fn changes_lints(new_text: &str, base_text: Option<&str>) -> bool {
    let new_lints = lint_tables(new_text);
    match base_text {
        Some(base) => lint_tables(base) != new_lints,
        None => !new_lints
            .as_ref()
            .is_some_and(only_inherits_workspace_lints),
    }
}

const ASSERTION_MARKERS: [&str; 2] = ["assert", "expect("];

const SCRIPT_SKIP_MARKERS: [&str; 5] = [".skip(", ".only(", ".todo(", "xit(", "xdescribe("];

const RUST_TEST_ATTRIBUTES: [&str; 2] = ["#[test]", "::test]"];

fn code_lines(path: &str, text: &str) -> Vec<String> {
    match Language::of_path(path) {
        Some(language) => lexer::scan(language, text).code_lines,
        None => text.lines().map(str::to_string).collect(),
    }
}

fn assertion_lines(path: &str, text: &str) -> BTreeSet<usize> {
    let mut lines = BTreeSet::new();
    let mut depth = 0usize;
    let mut inside = false;
    for (index, line) in code_lines(path, text).iter().enumerate() {
        let mut rest = line.as_str();
        if !inside {
            let Some(start) = ASSERTION_MARKERS
                .iter()
                .filter_map(|marker| line.find(marker))
                .min()
            else {
                continue;
            };
            rest = &line[start..];
            inside = true;
            depth = 0;
        }
        lines.insert(index + 1);
        let mut opened = depth > 0;
        for character in rest.chars() {
            match character {
                '(' | '[' | '{' => {
                    depth += 1;
                    opened = true;
                }
                ')' | ']' | '}' => depth = depth.saturating_sub(1),
                _ => {}
            }
            if opened && depth == 0 {
                break;
            }
        }
        if !opened || depth == 0 {
            inside = false;
        }
    }
    lines
}

fn count_in_code(path: &str, text: &str, needles: &[&str]) -> usize {
    code_lines(path, text)
        .iter()
        .filter(|line| needles.iter().any(|needle| line.contains(needle)))
        .count()
}

fn adds_skip(path: &str, diff: &LineDiff, is_test: bool) -> bool {
    let language = Language::of_path(path);
    diff.added.iter().any(|(_, line)| match language {
        Some(Language::Rust) => line.contains("#[") && line.contains("ignore"),
        Some(Language::Script) if is_test => SCRIPT_SKIP_MARKERS
            .iter()
            .any(|marker| line.contains(marker)),
        _ => false,
    })
}

fn rust_test_reasons(path: &str, new_text: &str, base_text: &str) -> Vec<String> {
    let mut reasons = Vec::new();
    if count_in_code(path, new_text, &RUST_TEST_ATTRIBUTES)
        < count_in_code(path, base_text, &RUST_TEST_ATTRIBUTES)
    {
        reasons.push("removes a test attribute".to_string());
    }
    if count_in_code(path, new_text, &["cfg(test)"])
        < count_in_code(path, base_text, &["cfg(test)"])
    {
        reasons.push("turns off test code".to_string());
    }
    reasons
}

pub(crate) fn oracle_reasons(
    change: &FileChange,
    diff: &LineDiff,
    new_text: Option<&str>,
    base_text: Option<&str>,
    tests: &GlobSet,
) -> Vec<String> {
    let mut reasons = Vec::new();
    let is_test = tests.is_match(&change.path);
    if change.is_deleted() {
        if is_test {
            reasons.push("deletes a test file".to_string());
        }
        return reasons;
    }
    if adds_skip(&change.path, diff, is_test) {
        reasons.push("skips a test".to_string());
    }
    if is_test {
        let assertions = base_text
            .map(|text| assertion_lines(&change.path, text))
            .unwrap_or_default();
        let touches_assertion = diff
            .removed
            .iter()
            .any(|(line, text)| assertions.contains(line) || text.contains("assert"));
        if touches_assertion {
            reasons.push("removes or changes an assertion".to_string());
        }
        if diff
            .added
            .iter()
            .any(|(_, line)| line.to_lowercase().contains("timeout"))
        {
            reasons.push("changes a test timeout".to_string());
        }
    }
    if let (Some(new), Some(base)) = (new_text, base_text)
        && Language::of_path(&change.path) == Some(Language::Rust)
    {
        reasons.extend(rust_test_reasons(&change.path, new, base));
    }
    if let Some(text) = new_text {
        let added: Vec<usize> = diff.added.iter().map(|(line, _)| *line).collect();
        let adds_suppression = suppressions_in(&change.path, text)
            .iter()
            .any(|(line, _)| added.contains(line));
        if adds_suppression {
            reasons.push("adds a lint or type suppression".to_string());
        }
        let is_manifest = change.path.rsplit('/').next() == Some("Cargo.toml");
        if is_manifest && changes_lints(text, base_text) {
            reasons.push("changes lint configuration".to_string());
        }
    }
    reasons
}

pub(crate) fn risk_tier(
    root: &Path,
    source: &DiffSource,
    settings: &Settings,
) -> Result<Vec<TierFinding>, String> {
    let map = TierMap::from_settings(settings)?;
    let tests = glob_set(&settings.behavior_tests)?;
    let mut findings = Vec::new();
    for change in git::changed_files(root, source)? {
        findings.push(map.classify(&change.path));
        let diff = if change.is_deleted() {
            LineDiff::default()
        } else {
            git::line_diff(root, source, &change.path)?
        };
        let new_text = if change.is_deleted() {
            None
        } else {
            git::new_content(root, source, &change.path).ok()
        };
        let base_text = git::base_content(root, source, &change.path);
        for reason in oracle_reasons(
            &change,
            &diff,
            new_text.as_deref(),
            base_text.as_deref(),
            &tests,
        ) {
            findings.push(TierFinding {
                tier: ORACLE_TIER,
                path: change.path.clone(),
                reason: format!("weakens a test or check: it {reason}"),
            });
        }
    }
    Ok(findings)
}

pub(crate) fn highest_tier(findings: &[TierFinding]) -> u8 {
    findings
        .iter()
        .map(|finding| finding.tier)
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
