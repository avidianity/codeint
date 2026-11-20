use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use super::{Violations, glob_set};
use crate::config::Settings;
use crate::git;

const EM_DASH: char = '\u{2014}';

pub(crate) fn agent_entry_links(root: &Path, settings: &Settings) -> Result<Violations, String> {
    let mut violations = Violations::new();
    let rules = &settings.rules_file;
    let rules_path = root.join(rules);
    let rules_is_file = fs::symlink_metadata(&rules_path).is_ok_and(|metadata| metadata.is_file());
    if !rules_is_file {
        violations.push(format!(
            "{rules} must exist as a regular file. It is the one rules file."
        ));
    }
    for entry in &settings.entry_files {
        let points_to_rules =
            fs::read_link(root.join(entry)).is_ok_and(|target| target == Path::new(rules));
        if !points_to_rules {
            violations.push(format!(
                "{entry} must be a link to {rules}. Edit {rules} only, then recreate the link."
            ));
        }
    }
    Ok(violations)
}

pub(crate) fn skills_link(root: &Path, settings: &Settings) -> Result<Violations, String> {
    let mut violations = Violations::new();
    let folder = root.join(&settings.skills_folder);
    let folder_is_real = fs::symlink_metadata(&folder).is_ok_and(|metadata| metadata.is_dir());
    if !folder_is_real {
        violations.push(format!(
            "{} must be a real folder. It is the one skills folder.",
            settings.skills_folder
        ));
        return Ok(violations);
    }
    let canonical_folder = folder
        .canonicalize()
        .map_err(|error| format!("cannot resolve {}: {error}", settings.skills_folder))?;
    for link in &settings.skills_links {
        let link_path = root.join(link);
        let is_link = fs::symlink_metadata(&link_path)
            .is_ok_and(|metadata| metadata.file_type().is_symlink());
        let resolves_to_folder = link_path
            .canonicalize()
            .is_ok_and(|resolved| resolved == canonical_folder);
        if !is_link || !resolves_to_folder {
            violations.push(format!(
                "{link} must be a link to {}.",
                settings.skills_folder
            ));
        }
    }
    let lock_path = root.join(&settings.skills_lock_file);
    let lock_text = fs::read_to_string(&lock_path)
        .map_err(|error| format!("cannot read {}: {error}", settings.skills_lock_file))?;
    let lock: serde_json::Value = serde_json::from_str(&lock_text)
        .map_err(|error| format!("{} is not valid JSON: {error}", settings.skills_lock_file))?;
    let pinned: BTreeSet<String> = lock
        .get("skills")
        .and_then(serde_json::Value::as_object)
        .map(|skills| skills.keys().cloned().collect())
        .unwrap_or_default();
    let installed: BTreeSet<String> = fs::read_dir(&folder)
        .map_err(|error| format!("cannot list {}: {error}", settings.skills_folder))?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    for skill in &installed {
        if !pinned.contains(skill) && !settings.project_skills.contains(skill) {
            violations.push(format!(
                "{skill} is not pinned in the skills lock file. Install it with the skills installer."
            ));
        }
    }
    for skill in pinned.difference(&installed) {
        violations.push(format!(
            "{skill} is pinned in {} but missing from {}. Restore it with the skills installer.",
            settings.skills_lock_file, settings.skills_folder
        ));
    }
    Ok(violations)
}

pub(crate) fn always_read_pages(rules_text: &str) -> Vec<String> {
    let mut pages = Vec::new();
    let mut column = None;
    for line in rules_text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            column = None;
            continue;
        }
        let cells: Vec<&str> = trimmed
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        if let Some(found) = cells.iter().position(|cell| *cell == "Always read") {
            column = Some(found);
            continue;
        }
        let Some(index) = column else { continue };
        let Some(cell) = cells.get(index) else {
            continue;
        };
        pages.extend(
            cell.split('`')
                .enumerate()
                .filter(|(position, text)| position % 2 == 1 && !text.is_empty())
                .map(|(_, text)| text.to_string()),
        );
    }
    pages.sort();
    pages.dedup();
    pages
}

pub(crate) fn doc_size(root: &Path, settings: &Settings) -> Result<Violations, String> {
    let rules_text = fs::read_to_string(root.join(&settings.rules_file))
        .map_err(|error| format!("cannot read {}: {error}", settings.rules_file))?;
    let mut pages = vec![settings.rules_file.clone()];
    pages.extend(always_read_pages(&rules_text));
    let mut violations = Violations::new();
    for page in pages {
        let size = fs::metadata(root.join(&page))
            .map_err(|error| format!("cannot read always-read page {page}: {error}"))?
            .len();
        if size >= settings.size_budget_bytes {
            violations.push(format!(
                "{page} is {size} bytes; the budget is {}. Split it or move detail to a reference page. Never raise the budget.",
                settings.size_budget_bytes
            ));
        }
    }
    Ok(violations)
}

fn readable_markdown(root: &Path, settings: &Settings) -> Result<Vec<String>, String> {
    Ok(git::tracked_files(root)?
        .into_iter()
        .filter(|path| path.ends_with(".md") || *path == settings.rules_file)
        .filter(|path| !settings.is_third_party_skill(path))
        .filter(|path| {
            fs::symlink_metadata(root.join(path)).is_ok_and(|metadata| metadata.is_file())
        })
        .collect())
}

pub(crate) fn doc_style(root: &Path, settings: &Settings) -> Result<Violations, String> {
    let mut violations = Violations::new();
    for path in readable_markdown(root, settings)? {
        let text = fs::read_to_string(root.join(&path))
            .map_err(|error| format!("cannot read {path}: {error}"))?;
        for (index, line) in text.lines().enumerate() {
            if line.contains(EM_DASH) {
                violations.push(format!(
                    "{path}:{} uses an em dash. Use a plain dash \"-\" or split the sentence.",
                    index + 1
                ));
            }
        }
    }
    Ok(violations)
}

pub(crate) fn placeholders_in(text: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    let mut fence: Option<bool> = None;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = match fence {
                Some(_) => None,
                None => Some(
                    trimmed
                        .trim_start_matches(['`', '~'])
                        .trim()
                        .starts_with("template"),
                ),
            };
            continue;
        }
        if fence == Some(true) {
            continue;
        }
        let mut rest = line;
        while let Some(start) = rest.find('<') {
            let after = &rest[start + 1..];
            let Some(end) = after.find(['>', '<']) else {
                break;
            };
            let inner = &after[..end];
            let closes = after[end..].starts_with('>');
            let starts_lowercase = inner
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_lowercase());
            let is_autolink = inner.contains("://") || inner.contains('@');
            if closes && starts_lowercase && !is_autolink {
                found.push((index + 1, inner.to_string()));
            }
            rest = &after[end..];
        }
    }
    found
}

pub(crate) fn no_placeholders(root: &Path, settings: &Settings) -> Result<Violations, String> {
    let scope = glob_set(&settings.placeholder_paths)?;
    let mut violations = Violations::new();
    for path in git::tracked_files(root)? {
        if !scope.is_match(&path)
            || !fs::symlink_metadata(root.join(&path)).is_ok_and(|metadata| metadata.is_file())
        {
            continue;
        }
        let text = fs::read_to_string(root.join(&path))
            .map_err(|error| format!("cannot read {path}: {error}"))?;
        for (line, placeholder) in placeholders_in(&text) {
            violations.push(format!(
                "{path}:{line} still holds the placeholder <{placeholder}>. Replace it with the Project value."
            ));
        }
    }
    Ok(violations)
}

pub(crate) fn record_numbers(root: &Path, settings: &Settings) -> Result<Violations, String> {
    let mut violations = Violations::new();
    for folder in &settings.record_folders {
        let Ok(entries) = fs::read_dir(root.join(folder)) else {
            continue;
        };
        let mut by_number: BTreeMap<u32, Vec<String>> = BTreeMap::new();
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().into_owned();
            let digits: String = name.chars().take_while(char::is_ascii_digit).collect();
            if digits.len() == 4 && name[4..].starts_with('-') && name.ends_with(".md") {
                let number: u32 = digits.parse().unwrap_or(0);
                by_number
                    .entry(number)
                    .or_default()
                    .push(format!("{folder}/{name}"));
            }
        }
        let next_free = by_number.keys().max().copied().unwrap_or(0) + 1;
        for (number, files) in &by_number {
            let mut files = files.clone();
            files.sort();
            if let [first, second, ..] = files.as_slice() {
                violations.push(format!(
                    "Record number {number:04} is used by {first} and {second}. Renumber the newer one to {next_free:04}."
                ));
            }
        }
        for expected in 1..next_free {
            if !by_number.contains_key(&expected) {
                violations.push(format!(
                    "Record number {expected:04} is missing in {folder}. Renumber the records so the sequence has no gaps."
                ));
            }
        }
    }
    Ok(violations)
}

#[cfg(test)]
mod tests;
