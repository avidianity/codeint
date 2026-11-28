use std::collections::{BTreeMap, HashMap, HashSet};
use std::string::FromUtf8Error;
use std::sync::RwLock;

use codeint_config::{ConfigFileKind, LanguageCoverage, SqlDialect};
use codeint_git::{BlobId, Snapshot};
use codeint_model::{
    CodeModel, LowerError, RepoPath, SkippedFile, SourceFile, Workspace, settle_source_set,
    validate_hazard_lines,
};
use codeint_plugin_host::{NamedLanguagePlugin, SqlParsedFile};
use codeint_report::RejectedStatementEntry;
use rayon::prelude::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};

use super::errors::CheckError;
use super::sql;

#[derive(Clone, Copy)]
pub(crate) struct VersionedLanguagePlugin<'a> {
    pub(crate) named: NamedLanguagePlugin<'a>,
    pub(crate) version: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct LoweringCacheKey {
    path: RepoPath,
    blob_id: BlobId,
    plugin_name: String,
    plugin_version: String,
    sql_dialect: Option<SqlDialect>,
}

#[derive(Default)]
pub(crate) struct LoweringCache {
    entries: RwLock<HashMap<LoweringCacheKey, LoweredOutcome>>,
}

impl LoweringCache {
    pub(crate) fn new() -> LoweringCache {
        LoweringCache::default()
    }

    fn get(&self, key: &LoweringCacheKey) -> Option<LoweredOutcome> {
        let entries = self
            .entries
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        entries.get(key).cloned()
    }

    fn insert(&self, key: LoweringCacheKey, value: LoweredOutcome) {
        let mut entries = self
            .entries
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        entries.insert(key, value);
    }
}

pub(super) struct CollectedPluginModels {
    pub(super) models: Vec<CodeModel>,
    pub(super) file_line_counts: BTreeMap<RepoPath, usize>,
    pub(super) sql_parsed: BTreeMap<RepoPath, SqlParsedFile>,
    pub(super) rejected_statements: Vec<RejectedStatementEntry>,
}

#[derive(Clone)]
pub(super) struct LoweredOutcome {
    file: LoweredFile,
    line_count: Option<(RepoPath, usize)>,
    sql: Option<sql::SqlLowerDetails>,
}

#[derive(Clone)]
enum LoweredFile {
    Measured(SourceFile),
    DeclarationOnly(RepoPath),
    Skipped(SkippedFile),
}

pub(super) fn assign_files_to_plugins<'tree>(
    tree: &'tree Snapshot,
    plugins: &[NamedLanguagePlugin<'_>],
    coverage: &[LanguageCoverage<'_>],
    config_file: ConfigFileKind,
) -> Result<Vec<(&'tree RepoPath, usize)>, CheckError> {
    let mut plugins_by_extension: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, named) in plugins.iter().enumerate() {
        for extension in named.plugin.extensions() {
            let claimants = plugins_by_extension.entry(extension).or_default();
            if !claimants.contains(&index) {
                claimants.push(index);
            }
        }
    }
    for (extension, claimants) in &plugins_by_extension {
        if claimants.len() < 2 {
            continue;
        }
        let unrestricted: Vec<usize> = claimants
            .iter()
            .copied()
            .filter(|&index| coverage[index].covers_all())
            .collect();
        if unrestricted.len() >= 2 {
            let (first, second) = first_two_plugin_names(plugins, &unrestricted);
            return Err(CheckError::ExtensionConflict {
                config_file,
                first,
                second,
                extension: extension.clone(),
                path: None,
            });
        }
    }
    let mut covered_files = Vec::new();
    for path in tree.files() {
        let Some(extension) = path.extension() else {
            continue;
        };
        let Some(claimants) = plugins_by_extension.get(extension) else {
            continue;
        };
        let matching: Vec<usize> = claimants
            .iter()
            .copied()
            .filter(|&index| coverage[index].covers(path))
            .collect();
        match matching.as_slice() {
            [] => {}
            [index] => covered_files.push((path, *index)),
            many => {
                let (first, second) = first_two_plugin_names(plugins, many);
                return Err(CheckError::ExtensionConflict {
                    config_file,
                    first,
                    second,
                    extension: extension.to_owned(),
                    path: Some(path.clone()),
                });
            }
        }
    }
    Ok(covered_files)
}

fn first_two_plugin_names(
    plugins: &[NamedLanguagePlugin<'_>],
    indices: &[usize],
) -> (String, String) {
    let mut names: Vec<&str> = indices.iter().map(|&index| plugins[index].name).collect();
    names.sort_unstable();
    (names[0].to_owned(), names[1].to_owned())
}

fn listed_hazard_rules_by_plugin(plugins: &[VersionedLanguagePlugin<'_>]) -> Vec<HashSet<String>> {
    plugins
        .iter()
        .map(|entry| {
            if entry.named.plugin.has_hazard_rules() {
                entry.named.plugin.hazard_rules().into_iter().collect()
            } else {
                HashSet::new()
            }
        })
        .collect()
}

pub(super) fn lower_covered_files(
    tree: &Snapshot,
    plugins: &[VersionedLanguagePlugin<'_>],
    covered_files: &[(&RepoPath, usize)],
    sql_dialects: &[Option<SqlDialect>],
    config_file: ConfigFileKind,
    lowering_cache: Option<&LoweringCache>,
) -> Result<Vec<(usize, LoweredOutcome)>, CheckError> {
    let listed_hazard_rules = listed_hazard_rules_by_plugin(plugins);
    let reader = tree.parallel_reader();
    covered_files
        .par_iter()
        .zip(sql_dialects.par_iter())
        .map_init(
            || reader.thread_local(),
            |local_reader, (&(path, index), &dialect)| {
                let plugin = plugins[index];
                let cache_key = lowering_cache.and_then(|_| {
                    local_reader.blob_id(path).map(|blob_id| LoweringCacheKey {
                        path: path.clone(),
                        blob_id,
                        plugin_name: plugin.named.name.to_owned(),
                        plugin_version: plugin.version.to_owned(),
                        sql_dialect: dialect,
                    })
                });
                if let (Some(cache), Some(key)) = (lowering_cache, cache_key.as_ref())
                    && let Some(cached) = cache.get(key)
                {
                    return Ok((index, cached));
                }
                let bytes = local_reader.read(path)?;
                let lowered = lower_bytes(
                    bytes,
                    path,
                    &plugin.named,
                    dialect,
                    &listed_hazard_rules[index],
                    config_file,
                )?;
                if let (Some(cache), Some(key)) = (lowering_cache, cache_key) {
                    cache.insert(key, lowered.clone());
                }
                Ok((index, lowered))
            },
        )
        .collect()
}

pub(super) fn collect_plugin_models(
    plugin_count: usize,
    workspace: Workspace,
    outcomes: Vec<(usize, LoweredOutcome)>,
) -> CollectedPluginModels {
    let mut models: Vec<CodeModel> = (0..plugin_count)
        .map(|_| CodeModel {
            workspace: workspace.clone(),
            ..CodeModel::default()
        })
        .collect();
    let mut file_line_counts = BTreeMap::new();
    let mut sql_parsed = BTreeMap::new();
    let mut rejected_statements = Vec::new();
    for (index, outcome) in outcomes {
        if let Some((path, lines)) = outcome.line_count {
            file_line_counts.insert(path, lines);
        }
        let model = &mut models[index];
        match outcome.file {
            LoweredFile::Measured(file) => {
                if let Some(details) = outcome.sql {
                    rejected_statements.extend(details.rejected_statements);
                    sql_parsed.insert(file.path.clone(), details.parsed);
                }
                model.files.push(file);
            }
            LoweredFile::DeclarationOnly(path) => model.declaration_only.push(path),
            LoweredFile::Skipped(skipped) => model.skipped.push(skipped),
        }
    }
    rejected_statements.sort_by(|a, b| {
        (a.path.as_str(), a.line, a.dialect.as_str()).cmp(&(
            b.path.as_str(),
            b.line,
            b.dialect.as_str(),
        ))
    });
    CollectedPluginModels {
        models,
        file_line_counts,
        sql_parsed,
        rejected_statements,
    }
}

fn lower_bytes(
    bytes: Vec<u8>,
    path: &RepoPath,
    named: &NamedLanguagePlugin<'_>,
    sql_dialect: Option<SqlDialect>,
    listed_hazard_rules: &HashSet<String>,
    config_file: ConfigFileKind,
) -> Result<LoweredOutcome, CheckError> {
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => {
            return Ok(LoweredOutcome {
                file: LoweredFile::Skipped(not_utf8(path.clone(), &error)),
                line_count: None,
                sql: None,
            });
        }
    };
    let line_count = text.lines().count();
    let recorded_lines = Some((path.clone(), line_count));
    let lowered = match sql_dialect {
        Some(dialect) => {
            sql::lower_sql_source(path, &text, dialect).map(|(file, details)| (file, Some(details)))
        }
        None => named
            .plugin
            .lower(path, &text, None)
            .map(|file| (file, None)),
    };
    match lowered {
        Ok((mut file, sql_details)) => {
            validate_hazard_lines(
                named.name,
                path,
                line_count,
                &file.hazard_lines,
                listed_hazard_rules,
            )
            .map_err(|error| CheckError::language_plugin(config_file, error))?;
            for function in &mut file.functions {
                function.coverage = None;
            }
            if !named.plugin.has_imports() {
                file.program_start = false;
            }
            file.source_set = settle_source_set(path, &text, file.source_set);
            Ok(LoweredOutcome {
                file: LoweredFile::Measured(SourceFile {
                    path: path.clone(),
                    model: file,
                }),
                line_count: recorded_lines,
                sql: sql_details,
            })
        }
        Err(LowerError::DeclarationOnly) => Ok(LoweredOutcome {
            file: LoweredFile::DeclarationOnly(path.clone()),
            line_count: recorded_lines,
            sql: None,
        }),
        Err(reason) => Ok(LoweredOutcome {
            file: LoweredFile::Skipped(SkippedFile {
                path: path.clone(),
                reason: sql::format_lower_error(sql_dialect.is_some(), reason),
            }),
            line_count: recorded_lines,
            sql: None,
        }),
    }
}

fn not_utf8(path: RepoPath, error: &FromUtf8Error) -> SkippedFile {
    SkippedFile {
        path,
        reason: format!("not valid UTF-8: {}", error.utf8_error()),
    }
}
