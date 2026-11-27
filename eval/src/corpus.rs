use std::collections::BTreeSet;
use std::path::Path;

use codeint_plugin_host::built_in_language_names;
use serde::{Deserialize, Serialize};

use crate::error::{EvaluationError, io_error};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    pub measured_on: String,
    pub repositories: Vec<CorpusEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub case_studies: Vec<CaseStudy>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseStudy {
    pub name: String,
    pub url: String,
    pub before: String,
    pub after: String,
    pub change: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusEntry {
    pub name: String,
    pub language: String,
    pub bucket: SizeBucket,
    pub url: String,
    pub commit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SizeBucket {
    Small,
    Medium,
    Large,
}

impl SizeBucket {
    pub fn for_production_lines(lines: u64) -> Option<SizeBucket> {
        match lines {
            0..2_000 => None,
            2_000..10_000 => Some(SizeBucket::Small),
            10_000..50_000 => Some(SizeBucket::Medium),
            _ => Some(SizeBucket::Large),
        }
    }
}

pub fn load_corpus(path: &Path) -> Result<Corpus, EvaluationError> {
    let text = std::fs::read_to_string(path).map_err(io_error(path))?;
    let corpus: Corpus =
        serde_json::from_str(&text).map_err(|error| manifest_error(path, error.to_string()))?;
    let mut names = BTreeSet::new();
    for entry in &corpus.repositories {
        validate_entry(entry).map_err(|message| manifest_error(path, message))?;
        if !names.insert((
            entry.name.as_str(),
            entry.language.as_str(),
            entry.panel.as_deref(),
        )) {
            return Err(manifest_error(
                path,
                format!("repository {} is listed twice", entry.label()),
            ));
        }
    }
    for study in &corpus.case_studies {
        validate_case_study(study).map_err(|message| manifest_error(path, message))?;
    }
    Ok(corpus)
}

fn manifest_error(path: &Path, message: String) -> EvaluationError {
    EvaluationError::Manifest {
        path: path.to_path_buf(),
        message,
    }
}

impl CorpusEntry {
    pub fn label(&self) -> String {
        let scope = self.panel.as_deref().unwrap_or(self.language.as_str());
        format!("{} ({scope})", self.name)
    }
}

fn is_full_sha(commit: &str) -> bool {
    commit.len() == 40
        && commit
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn is_safe_url(url: &str) -> bool {
    !url.is_empty()
        && !url.starts_with('-')
        && !url.chars().any(|ch| ch.is_whitespace() || ch == '\0')
}

fn validate_case_study(study: &CaseStudy) -> Result<(), String> {
    if study.name.is_empty() || !is_safe_url(&study.url) {
        return Err("every case study needs a name and a safe non-option url".to_owned());
    }
    for commit in [&study.before, &study.after] {
        if !is_full_sha(commit) {
            return Err(format!(
                "case study {} must pin both commits to 40 lowercase hexadecimal digits",
                study.name
            ));
        }
    }
    Ok(())
}

fn validate_entry(entry: &CorpusEntry) -> Result<(), String> {
    if entry.name.is_empty() || !is_safe_url(&entry.url) {
        return Err("every repository needs a name and a safe non-option url".to_owned());
    }
    if !built_in_language_names().contains(&entry.language.as_str()) {
        return Err(format!(
            "repository {} names {}, which is not a built-in Language plugin",
            entry.name, entry.language
        ));
    }
    if !is_full_sha(&entry.commit) {
        return Err(format!(
            "repository {} must pin commit to 40 lowercase hexadecimal digits",
            entry.name
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
