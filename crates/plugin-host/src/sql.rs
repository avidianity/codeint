use std::collections::BTreeMap;

use codeint_config::SqlDialect as ConfigSqlDialect;
use codeint_lang_sql::schema::{SqlSchema, collect_schema_hazards};
use codeint_lang_sql::{ParsedSqlStatement, Sql, SqlDialect};
use codeint_model::language::{
    DeclaredDep, FileModel, FrameworkProfileResult, FrameworkProfileSelection, ImportRequest,
    Manifest, Resolution,
};
use codeint_model::{
    CodeModel, LanguagePlugin, LanguagePluginError, LowerError, Package, ReferenceRanges, RepoPath,
    SourceFile, SourceSet, WorkspaceError,
};

pub(crate) struct BuiltInSql;

impl LanguagePlugin for BuiltInSql {
    fn extensions(&self) -> Vec<String> {
        Sql::new().extensions()
    }

    fn lower(&self, path: &RepoPath, text: &str) -> Result<FileModel, LowerError> {
        Sql::new().lower(path, text)
    }

    fn manifest_paths(
        &self,
        repository_paths: &[RepoPath],
    ) -> Result<Vec<String>, LanguagePluginError> {
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
        active_profiles: &[String],
    ) -> Result<Vec<Resolution>, LanguagePluginError> {
        Sql::new().resolve_imports(repository_paths, manifests, requests, active_profiles)
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

    fn reference_ranges(&self) -> Result<ReferenceRanges, LanguagePluginError> {
        Sql::new().reference_ranges()
    }

    fn known_framework_profiles(&self) -> Vec<String> {
        Sql::new().known_framework_profiles()
    }

    fn resolve_framework_profiles(
        &self,
        package_root: &str,
        files: &[RepoPath],
        manifests: Vec<Manifest>,
        selection: FrameworkProfileSelection,
    ) -> Result<FrameworkProfileResult, LanguagePluginError> {
        Sql::new().resolve_framework_profiles(package_root, files, manifests, selection)
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlRejectedStatement {
    pub path: String,
    pub line: u32,
    pub dialect: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlSchemaScopeSummary {
    pub package: Option<String>,
    pub dialect: String,
    pub tables: usize,
}

#[derive(Clone, Debug)]
pub struct SqlParsedFile {
    dialect: SqlDialect,
    statements: Vec<ParsedSqlStatement>,
}

#[derive(Debug)]
pub struct LoweredSqlFile {
    pub file_model: FileModel,
    pub parsed: SqlParsedFile,
    pub rejected_statements: Vec<SqlRejectedStatement>,
}

fn to_plugin_dialect(dialect: ConfigSqlDialect) -> SqlDialect {
    match dialect {
        ConfigSqlDialect::Generic => SqlDialect::Generic,
        ConfigSqlDialect::Postgresql => SqlDialect::Postgresql,
        ConfigSqlDialect::Mysql => SqlDialect::Mysql,
        ConfigSqlDialect::Sqlite => SqlDialect::Sqlite,
        ConfigSqlDialect::Duckdb => SqlDialect::Duckdb,
    }
}

pub fn lower_sql_source(
    path: &RepoPath,
    text: &str,
    dialect: ConfigSqlDialect,
) -> Result<LoweredSqlFile, LowerError> {
    let plugin_dialect = to_plugin_dialect(dialect);
    let output = Sql::with_dialect(plugin_dialect).lower_with_details(path, text)?;
    let rejected_statements = output
        .rejected_statements
        .into_iter()
        .map(|entry| SqlRejectedStatement {
            path: entry.path.to_string(),
            line: entry.line,
            dialect: entry.dialect.display_name().to_owned(),
        })
        .collect();
    Ok(LoweredSqlFile {
        file_model: output.file_model,
        parsed: SqlParsedFile {
            dialect: plugin_dialect,
            statements: output.parsed_statements,
        },
        rejected_statements,
    })
}

pub fn apply_sql_schema_pass(
    sql_model: &mut CodeModel,
    parsed_by_path: &BTreeMap<RepoPath, SqlParsedFile>,
) -> Vec<SqlSchemaScopeSummary> {
    let mut scopes: BTreeMap<(Option<RepoPath>, SqlDialect), Vec<usize>> = BTreeMap::new();
    for (index, file) in sql_model.files.iter().enumerate() {
        let Some(parsed) = parsed_by_path.get(&file.path) else {
            continue;
        };
        let package = match sql_model.workspace.package_of(&file.path) {
            Package::Root => None,
            Package::Member(root) => Some(root.clone()),
        };
        scopes
            .entry((package, parsed.dialect))
            .or_default()
            .push(index);
    }

    let mut summaries = Vec::with_capacity(scopes.len());
    for ((package, dialect), mut indices) in scopes {
        indices.sort_by(|&a, &b| sql_model.files[a].path.cmp(&sql_model.files[b].path));
        let schema = build_scope_schema(&sql_model.files, &indices, parsed_by_path);
        summaries.push(SqlSchemaScopeSummary {
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
    files: &[SourceFile],
    indices: &[usize],
    parsed_by_path: &BTreeMap<RepoPath, SqlParsedFile>,
) -> SqlSchema {
    let schema_inputs = indices.iter().filter_map(|&index| {
        let file = &files[index];
        if file.model.source_set == SourceSet::Vendored || is_down_migration_path(&file.path) {
            return None;
        }
        parsed_by_path
            .get(&file.path)
            .map(|parsed| parsed.statements.as_slice())
    });
    SqlSchema::build(schema_inputs)
}

fn is_down_migration_path(path: &RepoPath) -> bool {
    let file_name = path.as_str().rsplit('/').next().unwrap_or(path.as_str());
    file_name.eq_ignore_ascii_case("down.sql")
        || file_name.to_ascii_lowercase().ends_with(".down.sql")
}

fn apply_scope_schema_hazards(
    files: &mut [SourceFile],
    indices: &[usize],
    parsed_by_path: &BTreeMap<RepoPath, SqlParsedFile>,
    schema: &SqlSchema,
    dialect: SqlDialect,
) {
    for &index in indices {
        let file = &mut files[index];
        let Some(parsed) = parsed_by_path.get(&file.path) else {
            continue;
        };
        let extra = collect_schema_hazards(&parsed.statements, schema, dialect);
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
