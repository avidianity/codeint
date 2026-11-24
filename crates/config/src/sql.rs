use std::fmt;

use codeint_model::RepoPath;
use globset::GlobSet;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config_error::{ConfigFileKind, ConfigurationFileError, KeyPath, KeySegment, glob_set};

pub const KNOWN_SQL_DIALECTS: [&str; 4] = ["postgresql", "mysql", "sqlite", "duckdb"];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SqlDialect {
    #[default]
    Generic,
    Postgresql,
    Mysql,
    Sqlite,
    Duckdb,
}

impl SqlDialect {
    pub fn as_str(self) -> &'static str {
        match self {
            SqlDialect::Generic => "generic",
            SqlDialect::Postgresql => "postgresql",
            SqlDialect::Mysql => "mysql",
            SqlDialect::Sqlite => "sqlite",
            SqlDialect::Duckdb => "duckdb",
        }
    }

    pub fn parse_configured(name: &str) -> Option<SqlDialect> {
        match name {
            "postgresql" => Some(SqlDialect::Postgresql),
            "mysql" => Some(SqlDialect::Mysql),
            "sqlite" => Some(SqlDialect::Sqlite),
            "duckdb" => Some(SqlDialect::Duckdb),
            _ => None,
        }
    }
}

impl fmt::Display for SqlDialect {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "{path} matches conflicting SQL dialects: {first_dialect} (pattern {first_pattern:?}) and {second_dialect} (pattern {second_pattern:?})"
)]
pub struct SqlDialectConflict {
    pub path: RepoPath,
    pub first_dialect: SqlDialect,
    pub first_pattern: String,
    pub second_dialect: SqlDialect,
    pub second_pattern: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ConfiguredSqlDialect {
    Postgresql,
    Mysql,
    Sqlite,
    Duckdb,
}

impl ConfiguredSqlDialect {
    fn into_dialect(self) -> SqlDialect {
        match self {
            ConfiguredSqlDialect::Postgresql => SqlDialect::Postgresql,
            ConfiguredSqlDialect::Mysql => SqlDialect::Mysql,
            ConfiguredSqlDialect::Sqlite => SqlDialect::Sqlite,
            ConfiguredSqlDialect::Duckdb => SqlDialect::Duckdb,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SqlDialectEntryFile {
    #[schemars(
        description = "SQL Dialect for matching files: postgresql, mysql (including MariaDB), sqlite, or duckdb."
    )]
    dialect: ConfiguredSqlDialect,
    #[schemars(
        description = "Globs of repository-relative folders or files that use this SQL Dialect; a folder covers every file below it."
    )]
    include: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SqlConfigFile {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(
        description = "Folder or file patterns mapped to a SQL Dialect (postgresql, mysql, sqlite, or duckdb). Files covered by no entry use generic SQL."
    )]
    pub(crate) dialects: Vec<SqlDialectEntryFile>,
}

impl SqlConfigFile {
    pub(crate) fn is_unset(&self) -> bool {
        self.dialects.is_empty()
    }
}

#[derive(Debug)]
struct SqlDialectEntry {
    dialect: SqlDialect,
    patterns: Vec<String>,
    include: GlobSet,
}

#[derive(Debug, Default)]
pub struct SqlConfig {
    entries: Vec<SqlDialectEntry>,
}

impl SqlConfig {
    pub(crate) fn from_file(file: SqlConfigFile) -> Result<SqlConfig, ConfigurationFileError> {
        let dialects_key = KeyPath::nested("sql", "dialects");
        let entries = file
            .dialects
            .into_iter()
            .enumerate()
            .map(|(index, entry)| compile_dialect_entry(&dialects_key, index, entry))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SqlConfig { entries })
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn dialect_for(&self, path: &RepoPath) -> Result<SqlDialect, SqlDialectConflict> {
        if self.entries.is_empty() {
            return Ok(SqlDialect::Generic);
        }
        let candidates = path_and_ancestors(path);
        let mut matched: Option<(SqlDialect, &str)> = None;
        for entry in &self.entries {
            let Some(pattern) = first_matching_pattern(entry, &candidates) else {
                continue;
            };
            match matched {
                None => matched = Some((entry.dialect, pattern)),
                Some((first_dialect, _)) if first_dialect == entry.dialect => {}
                Some((first_dialect, first_pattern)) => {
                    return Err(SqlDialectConflict {
                        path: path.clone(),
                        first_dialect,
                        first_pattern: first_pattern.to_owned(),
                        second_dialect: entry.dialect,
                        second_pattern: pattern.to_owned(),
                    });
                }
            }
        }
        Ok(matched.map_or(SqlDialect::Generic, |(dialect, _)| dialect))
    }
}

fn compile_dialect_entry(
    dialects_key: &KeyPath,
    index: usize,
    entry: SqlDialectEntryFile,
) -> Result<SqlDialectEntry, ConfigurationFileError> {
    let dialect = entry.dialect.into_dialect();
    let include_key = dialects_key
        .child(KeySegment::Index(index))
        .child(KeySegment::Key("include".to_owned()));
    if entry.include.is_empty() {
        return Err(ConfigurationFileError::EmptySqlDialectInclude {
            file: ConfigFileKind::Json,
            key: include_key,
            dialect: dialect.as_str().to_owned(),
        });
    }
    for (pattern_index, pattern) in entry.include.iter().enumerate() {
        if pattern.trim().is_empty() {
            return Err(ConfigurationFileError::EmptySqlDialectPattern {
                file: ConfigFileKind::Json,
                key: include_key.child(KeySegment::Index(pattern_index)),
            });
        }
    }
    let include = glob_set(&include_key, &entry.include)?;
    Ok(SqlDialectEntry {
        dialect,
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
    entry: &'entry SqlDialectEntry,
    candidates: &[&str],
) -> Option<&'entry str> {
    for candidate in candidates {
        if let Some(index) = entry.include.matches(candidate).into_iter().min() {
            return Some(entry.patterns[index].as_str());
        }
    }
    None
}
