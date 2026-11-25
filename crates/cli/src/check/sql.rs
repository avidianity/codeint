use std::collections::{BTreeMap, HashSet};

use codeint_config::{ConfigFileKind, Configuration, SqlConfig, SqlDialect};
use codeint_model::language::FileModel;
use codeint_model::{CodeModel, LowerError, RepoPath, Workspace};
use codeint_plugin_host::{
    NamedLanguagePlugin, SQL_PLUGIN_NAME, SqlParsedFile,
    apply_sql_schema_pass as host_apply_sql_schema_pass, lower_sql_source as host_lower_sql_source,
};
use codeint_report::{RejectedStatementEntry, SqlSchemaSummary};

use super::{CheckError, SelectedPackage};

pub(crate) struct SqlLowerDetails {
    pub(crate) parsed: SqlParsedFile,
    pub(crate) rejected_statements: Vec<RejectedStatementEntry>,
}

pub(crate) fn resolve_sql_dialects(
    plugins: &[NamedLanguagePlugin<'_>],
    covered_files: &[(&RepoPath, usize)],
    sql_config: &SqlConfig,
    config_file: ConfigFileKind,
) -> Result<Vec<Option<SqlDialect>>, CheckError> {
    covered_files
        .iter()
        .map(|&(path, index)| {
            if plugins[index].name != SQL_PLUGIN_NAME {
                return Ok(None);
            }
            sql_config.dialect_for(path).map(Some).map_err(|conflict| {
                CheckError::SqlDialectConflict {
                    config_file,
                    conflict,
                }
            })
        })
        .collect()
}

pub(crate) fn lower_sql_source(
    path: &RepoPath,
    text: &str,
    dialect: SqlDialect,
) -> Result<(FileModel, SqlLowerDetails), LowerError> {
    let output = host_lower_sql_source(path, text, dialect)?;
    let rejected_statements = output
        .rejected_statements
        .into_iter()
        .map(|entry| RejectedStatementEntry {
            path: entry.path,
            line: entry.line,
            dialect: entry.dialect,
        })
        .collect();
    Ok((
        output.file_model,
        SqlLowerDetails {
            parsed: output.parsed,
            rejected_statements,
        },
    ))
}

pub(crate) fn format_lower_error(is_sql: bool, error: LowerError) -> String {
    if is_sql
        && let LowerError::Unparsable { ref message } = error
        && message == "templated SQL"
    {
        return message.clone();
    }
    error.to_string()
}

pub(crate) fn apply_sql_schema_pass(
    sql_model: &mut CodeModel,
    parsed_by_path: &BTreeMap<RepoPath, SqlParsedFile>,
) -> Vec<SqlSchemaSummary> {
    host_apply_sql_schema_pass(sql_model, parsed_by_path)
        .into_iter()
        .map(|summary| SqlSchemaSummary {
            package: summary.package,
            dialect: summary.dialect,
            tables: summary.tables,
        })
        .collect()
}

pub(crate) fn filter_sql_extras_to_package(
    rejected: &mut Vec<RejectedStatementEntry>,
    schemas: &mut Vec<SqlSchemaSummary>,
    workspace: &Workspace,
    selected: &SelectedPackage,
) {
    rejected.retain(|entry| {
        RepoPath::new(&entry.path)
            .ok()
            .is_some_and(|path| selected.matches(workspace.package_of(&path)))
    });
    schemas.retain(|summary| match (selected, summary.package.as_deref()) {
        (SelectedPackage::Root, None) => true,
        (SelectedPackage::Member(root), Some(pkg)) => pkg == root.as_str(),
        _ => false,
    });
}

pub(crate) fn validate_disabled_hazard_rules(
    plugins: &[NamedLanguagePlugin<'_>],
    configuration: &Configuration,
) -> Result<(), CheckError> {
    if configuration.hazards.disabled_rules.is_empty() {
        return Ok(());
    }
    let known: HashSet<String> = plugins
        .iter()
        .filter(|named| named.plugin.has_hazard_rules())
        .flat_map(|named| named.plugin.hazard_rules())
        .collect();
    for rule in &configuration.hazards.disabled_rules {
        if !known.contains(rule) {
            return Err(CheckError::UnknownHazardRule {
                config_file: configuration.config_file(),
                rule: rule.clone(),
            });
        }
    }
    Ok(())
}
