use std::collections::BTreeMap;

use codeint_model::RepoPath;
use globset::GlobSet;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config_error::{ConfigFileKind, ConfigurationFileError, KeyPath, KeySegment, glob_set};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "{path} matches conflicting {language} Dialects: {first_dialect} (pattern {first_pattern:?}) and {second_dialect} (pattern {second_pattern:?})"
)]
pub struct DialectConflict {
    pub language: String,
    pub path: RepoPath,
    pub first_dialect: String,
    pub first_pattern: String,
    pub second_dialect: String,
    pub second_pattern: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct DialectEntryFile {
    #[schemars(
        description = "Dialect name accepted by this Language plugin's known-dialects export."
    )]
    dialect: String,
    #[schemars(
        description = "Globs of repository-relative folders or files that use this Dialect; a folder covers every file below it."
    )]
    include: Vec<String>,
}

#[derive(Debug)]
struct DialectEntry {
    dialect: String,
    patterns: Vec<String>,
    include: GlobSet,
}

#[derive(Debug, Default)]
pub struct DialectRules {
    entries: Vec<DialectEntry>,
}

impl DialectRules {
    pub(crate) fn from_file(
        language: &str,
        entries: Vec<DialectEntryFile>,
    ) -> Result<DialectRules, ConfigurationFileError> {
        let language_key = KeyPath::nested("dialects", language);
        let compiled = entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| compile_dialect_entry(&language_key, language, index, entry))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(DialectRules { entries: compiled })
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn dialect_for(
        &self,
        language: &str,
        path: &RepoPath,
    ) -> Result<Option<&str>, Box<DialectConflict>> {
        if self.entries.is_empty() {
            return Ok(None);
        }
        let candidates = path_and_ancestors(path);
        let mut matched: Option<(&str, &str)> = None;
        for entry in &self.entries {
            let Some(pattern) = first_matching_pattern(entry, &candidates) else {
                continue;
            };
            match matched {
                None => matched = Some((entry.dialect.as_str(), pattern)),
                Some((first_dialect, _)) if first_dialect == entry.dialect => {}
                Some((first_dialect, first_pattern)) => {
                    return Err(Box::new(DialectConflict {
                        language: language.to_owned(),
                        path: path.clone(),
                        first_dialect: first_dialect.to_owned(),
                        first_pattern: first_pattern.to_owned(),
                        second_dialect: entry.dialect.clone(),
                        second_pattern: pattern.to_owned(),
                    }));
                }
            }
        }
        Ok(matched.map(|(dialect, _)| dialect))
    }

    fn validate_known(
        &self,
        config_file: ConfigFileKind,
        language: &str,
        known_dialects: &[String],
    ) -> Result<(), ConfigurationFileError> {
        let language_key = KeyPath::nested("dialects", language);
        for (index, entry) in self.entries.iter().enumerate() {
            if !known_dialects.iter().any(|known| known == &entry.dialect) {
                return Err(ConfigurationFileError::UnknownDialect {
                    file: config_file,
                    key: language_key
                        .child(KeySegment::Index(index))
                        .child(KeySegment::Key("dialect".to_owned())),
                    language: language.to_owned(),
                    dialect: entry.dialect.clone(),
                    known: known_dialects.to_vec(),
                });
            }
        }
        Ok(())
    }
}

pub(crate) fn parse_dialects_map(
    raw: BTreeMap<String, Vec<DialectEntryFile>>,
) -> Result<BTreeMap<String, DialectRules>, ConfigurationFileError> {
    let dialects_key = KeyPath::default().child(KeySegment::Key("dialects".to_owned()));
    let mut parsed = BTreeMap::new();
    for (language, entries) in raw {
        if language.trim().is_empty() {
            return Err(ConfigurationFileError::EmptyDialectLanguage {
                file: ConfigFileKind::Json,
                key: dialects_key,
            });
        }
        let rules = DialectRules::from_file(&language, entries)?;
        parsed.insert(language, rules);
    }
    Ok(parsed)
}

pub fn validate_dialects(
    config_file: ConfigFileKind,
    dialects: &BTreeMap<String, DialectRules>,
    known_by_language: &BTreeMap<String, Vec<String>>,
) -> Result<(), ConfigurationFileError> {
    let known_languages: Vec<String> = known_by_language.keys().cloned().collect();
    for (language, rules) in dialects {
        let Some(known_dialects) = known_by_language.get(language) else {
            return Err(ConfigurationFileError::UnknownDialectLanguage {
                file: config_file,
                key: KeyPath::nested("dialects", language),
                language: language.clone(),
                known: known_languages,
            });
        };
        rules.validate_known(config_file, language, known_dialects)?;
    }
    Ok(())
}

fn compile_dialect_entry(
    language_key: &KeyPath,
    language: &str,
    index: usize,
    entry: DialectEntryFile,
) -> Result<DialectEntry, ConfigurationFileError> {
    let entry_key = language_key.child(KeySegment::Index(index));
    if entry.dialect.trim().is_empty() {
        return Err(ConfigurationFileError::EmptyDialectName {
            file: ConfigFileKind::Json,
            key: entry_key.child(KeySegment::Key("dialect".to_owned())),
        });
    }
    let include_key = entry_key.child(KeySegment::Key("include".to_owned()));
    if entry.include.is_empty() {
        return Err(ConfigurationFileError::EmptyDialectInclude {
            file: ConfigFileKind::Json,
            key: include_key,
            language: language.to_owned(),
            dialect: entry.dialect,
        });
    }
    for (pattern_index, pattern) in entry.include.iter().enumerate() {
        if pattern.trim().is_empty() {
            return Err(ConfigurationFileError::EmptyDialectPattern {
                file: ConfigFileKind::Json,
                key: include_key.child(KeySegment::Index(pattern_index)),
            });
        }
    }
    let include = glob_set(&include_key, &entry.include)?;
    Ok(DialectEntry {
        dialect: entry.dialect,
        patterns: entry.include,
        include,
    })
}

fn path_and_ancestors(path: &RepoPath) -> Vec<&str> {
    path.as_str()
        .match_indices('/')
        .map(|(end, _)| &path.as_str()[..end])
        .chain([path.as_str()])
        .collect()
}

fn first_matching_pattern<'entry>(
    entry: &'entry DialectEntry,
    candidates: &[&str],
) -> Option<&'entry str> {
    for candidate in candidates {
        if let Some(index) = entry.include.matches(candidate).into_iter().min() {
            return Some(entry.patterns[index].as_str());
        }
    }
    None
}
