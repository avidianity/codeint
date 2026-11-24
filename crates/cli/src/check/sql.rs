use std::collections::{BTreeMap, HashSet};

use codeint_config::{ConfigFileKind, Configuration, SqlConfig, SqlDialect as ConfigSqlDialect};
use codeint_lang_sql::schema::{SqlSchema, collect_schema_hazards};
use codeint_lang_sql::{ParsedSqlStatement, Sql, SqlDialect};
use codeint_model::language::{DeclaredDep, FileModel, ImportRequest, Manifest, Resolution};
use codeint_model::{
    CodeModel, InvalidReferenceRange, LanguagePlugin, LanguagePluginError, LowerError, Package,
    ReferenceRanges, RepoPath, SourceSet, Workspace, WorkspaceError,
};
use codeint_report::{RejectedStatementEntry, SqlSchemaSummary};

use super::{CheckError, NamedLanguagePlugin, SelectedPackage};

pub(crate) const SQL_PLUGIN_NAME: &str = "sql";

pub(crate) struct BuiltInSql;

impl LanguagePlugin for BuiltInSql {
    fn extensions(&self) -> Vec<String> {
        Sql::new().extensions()
    }

    fn lower(&self, path: &RepoPath, text: &str) -> Result<FileModel, LowerError> {
        Sql::new().lower(path, text)
    }

    fn manifest_paths(&self, repository_paths: &[RepoPath]) -> Vec<String> {
        Sql::new().manifest_paths(repository_paths)
    }

    fn workspace_packages(
        &self,
        manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, WorkspaceError> {
        Sql::new().workspace_packages(manifests)
    }

    fn resolve_imports(
        &self,
        repository_paths: &[RepoPath],
        manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
    ) -> Result<Vec<Resolution>, LanguagePluginError> {
        Sql::new().resolve_imports(repository_paths, manifests, requests)
    }

    fn declared_dependencies(
        &self,
        manifests: Vec<Manifest>,
    ) -> Result<Vec<DeclaredDep>, LanguagePluginError> {
        Sql::new().declared_dependencies(manifests)
    }

    fn entry_points(
        &self,
        repository_paths: &[RepoPath],
        manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, LanguagePluginError> {
        Sql::new().entry_points(repository_paths, manifests)
    }

    fn reference_ranges(&self) -> Result<ReferenceRanges, InvalidReferenceRange> {
        Sql::new().reference_ranges()
    }

    fn has_hazard_rules(&self) -> bool {
        Sql::new().has_hazard_rules()
    }

    fn hazard_rules(&self) -> Vec<String> {
        Sql::new().hazard_rules()
    }

    fn has_imports(&self) -> bool {
        Sql::new().has_imports()
    }
}

pub(crate) struct SqlLowerDetails {
    pub(crate) dialect: SqlDialect,
    pub(crate) parsed_statements: Vec<ParsedSqlStatement>,
    pub(crate) rejected_statements: Vec<RejectedStatementEntry>,
}

pub(crate) fn to_plugin_dialect(dialect: ConfigSqlDialect) -> SqlDialect {
    match dialect {
        ConfigSqlDialect::Generic => SqlDialect::Generic,
        ConfigSqlDialect::Postgresql => SqlDialect::Postgresql,
        ConfigSqlDialect::Mysql => SqlDialect::Mysql,
        ConfigSqlDialect::Sqlite => SqlDialect::Sqlite,
        ConfigSqlDialect::Duckdb => SqlDialect::Duckdb,
    }
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
            sql_config
                .dialect_for(path)
                .map(|dialect| Some(to_plugin_dialect(dialect)))
                .map_err(|conflict| CheckError::SqlDialectConflict {
                    config_file,
                    conflict,
                })
        })
        .collect()
}

pub(crate) fn lower_sql_source(
    path: &RepoPath,
    text: &str,
    dialect: SqlDialect,
) -> Result<(FileModel, SqlLowerDetails), LowerError> {
    let output = Sql::with_dialect(dialect).lower_with_details(path, text)?;
    let rejected_statements = output
        .rejected_statements
        .into_iter()
        .map(|entry| RejectedStatementEntry {
            path: entry.path.to_string(),
            line: entry.line,
            dialect: entry.dialect.display_name().to_owned(),
        })
        .collect();
    Ok((
        output.file_model,
        SqlLowerDetails {
            dialect,
            parsed_statements: output.parsed_statements,
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
    parsed_by_path: &BTreeMap<RepoPath, (SqlDialect, Vec<ParsedSqlStatement>)>,
) -> Vec<SqlSchemaSummary> {
    let mut scopes: BTreeMap<(Option<RepoPath>, SqlDialect), Vec<usize>> = BTreeMap::new();
    for (index, file) in sql_model.files.iter().enumerate() {
        let Some(&(dialect, _)) = parsed_by_path.get(&file.path) else {
            continue;
        };
        let package = match sql_model.workspace.package_of(&file.path) {
            Package::Root => None,
            Package::Member(root) => Some(root.clone()),
        };
        scopes.entry((package, dialect)).or_default().push(index);
    }

    let mut summaries = Vec::with_capacity(scopes.len());
    for ((package, dialect), mut indices) in scopes {
        indices.sort_by(|&a, &b| sql_model.files[a].path.cmp(&sql_model.files[b].path));
        let schema = build_scope_schema(&sql_model.files, &indices, parsed_by_path);
        summaries.push(SqlSchemaSummary {
            package: package.map(|root| root.to_string()),
            dialect: dialect.display_name().to_owned(),
            tables: schema.table_count(),
        });
        if !schema.is_empty() {
            apply_scope_schema_hazards(
                &mut sql_model.files,
                &indices,
                parsed_by_path,
                &schema,
                dialect,
            );
        }
    }
    summaries.sort_by(|a, b| {
        (a.package.as_deref(), a.dialect.as_str()).cmp(&(b.package.as_deref(), b.dialect.as_str()))
    });
    summaries
}

fn build_scope_schema(
    files: &[codeint_model::SourceFile],
    indices: &[usize],
    parsed_by_path: &BTreeMap<RepoPath, (SqlDialect, Vec<ParsedSqlStatement>)>,
) -> SqlSchema {
    let schema_inputs = indices.iter().filter_map(|&index| {
        let file = &files[index];
        if file.model.source_set == SourceSet::Vendored || is_down_migration_path(&file.path) {
            return None;
        }
        parsed_by_path
            .get(&file.path)
            .map(|(_, parsed)| parsed.as_slice())
    });
    SqlSchema::build(schema_inputs)
}

fn is_down_migration_path(path: &RepoPath) -> bool {
    let file_name = path.as_str().rsplit('/').next().unwrap_or(path.as_str());
    file_name.eq_ignore_ascii_case("down.sql")
        || file_name.to_ascii_lowercase().ends_with(".down.sql")
}

fn apply_scope_schema_hazards(
    files: &mut [codeint_model::SourceFile],
    indices: &[usize],
    parsed_by_path: &BTreeMap<RepoPath, (SqlDialect, Vec<ParsedSqlStatement>)>,
    schema: &SqlSchema,
    dialect: SqlDialect,
) {
    for &index in indices {
        let file = &mut files[index];
        let Some((_, parsed)) = parsed_by_path.get(&file.path) else {
            continue;
        };
        let extra = collect_schema_hazards(parsed, schema, dialect);
        if extra.is_empty() {
            continue;
        }
        file.model.hazard_lines.extend(extra);
        file.model
            .hazard_lines
            .sort_by(|a, b| (a.line, &a.rule).cmp(&(b.line, &b.rule)));
        file.model.hazard_lines.dedup();
    }
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
