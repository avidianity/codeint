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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusEntry {
    pub name: String,
    pub language: String,
    pub bucket: SizeBucket,
    pub url: String,
    pub commit: String,
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
        if !names.insert(entry.name.as_str()) {
            return Err(manifest_error(
                path,
                format!("repository {} is listed twice", entry.name),
            ));
        }
    }
    Ok(corpus)
}

fn manifest_error(path: &Path, message: String) -> EvaluationError {
    EvaluationError::Manifest {
        path: path.to_path_buf(),
        message,
    }
}

fn validate_entry(entry: &CorpusEntry) -> Result<(), String> {
    if entry.name.is_empty() || entry.url.is_empty() {
        return Err("every repository needs a name and a url".to_owned());
    }
    if !built_in_language_names().contains(&entry.language.as_str()) {
        return Err(format!(
            "repository {} names {}, which is not a built-in Language plugin",
            entry.name, entry.language
        ));
    }
    let is_full_sha = entry.commit.len() == 40
        && entry
            .commit
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
    if !is_full_sha {
        return Err(format!(
            "repository {} must pin commit to 40 lowercase hexadecimal digits",
            entry.name
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
