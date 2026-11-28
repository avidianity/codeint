mod artifact;
mod builder;
mod codec;
mod expr;
mod rules;
mod scope;
#[cfg(test)]
mod tests;
mod types;

use codeint_model::{FileScopeArtifact, LanguagePluginError, RepoPath, ScopeAnalysis};

use crate::{ParsedSqlStatement, SqlDialect};

pub use rules::{collect_schema_hazards, schema_hazard_rules};
pub use types::{ColumnTypeClass, IndexSchema, SqlSchema, TableSchema};

pub(crate) fn encode_file_artifact(
    path: &RepoPath,
    statements: &[ParsedSqlStatement],
    dialect: SqlDialect,
) -> Vec<u8> {
    let is_down_migration = artifact::is_down_migration_path(path);
    let ddl_ops = builder::extract_ddl_ops(statements);
    let (scopes, checks) = rules::extract_scopes_and_checks(statements, dialect);
    let file_artifact = artifact::SqlFileArtifact {
        is_down_migration,
        ddl_ops,
        scopes,
        checks,
    };
    codec::encode_artifact(&file_artifact)
}

pub(crate) fn analyze_scope_artifacts(
    dialect: Option<&str>,
    artifacts: Vec<FileScopeArtifact>,
) -> Result<ScopeAnalysis, LanguagePluginError> {
    artifact::analyze_scope_artifacts(dialect, artifacts)
}
