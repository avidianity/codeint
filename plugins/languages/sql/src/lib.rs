#![forbid(unsafe_code)]

mod ast_loc;
mod flagged;
mod hazards;
mod parser;
pub mod schema;
mod source_set;
mod statements;
mod tokenize;

use std::collections::BTreeSet;
use std::fmt;

use codeint_model::language::{
    DeclaredDep, FileModel, Function, ImportRequest, Manifest, Resolution,
};
use codeint_model::{
    LanguagePlugin, LanguagePluginError, LowerError, ReferenceRanges, RepoPath, Token,
    WorkspaceError,
};

pub use flagged::flagged_line_rules;
pub use hazards::{SQL_HAZARD_RULES, hazard_line_rules};
pub use parser::{ParsedSqlStatement, RejectedSqlStatement};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SqlDialect {
    #[default]
    Generic,
    Postgresql,
    Mysql,
    Sqlite,
    Duckdb,
}

impl SqlDialect {
    const fn labels(self) -> (&'static str, &'static str) {
        match self {
            SqlDialect::Generic => ("generic", "generic SQL"),
            SqlDialect::Postgresql => ("postgresql", "postgresql"),
            SqlDialect::Mysql => ("mysql", "mysql"),
            SqlDialect::Sqlite => ("sqlite", "sqlite"),
            SqlDialect::Duckdb => ("duckdb", "duckdb"),
        }
    }

    pub fn as_str(self) -> &'static str {
        self.labels().0
    }

    pub fn display_name(self) -> &'static str {
        self.labels().1
    }
}

impl fmt::Display for SqlDialect {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.display_name())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlLowerOutput {
    pub file_model: FileModel,
    pub rejected_statements: Vec<RejectedSqlStatement>,
    pub parsed_statements: Vec<ParsedSqlStatement>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sql {
    dialect: SqlDialect,
}

impl Sql {
    pub fn new() -> Sql {
        Sql {
            dialect: SqlDialect::Generic,
        }
    }

    pub fn with_dialect(dialect: SqlDialect) -> Sql {
        Sql { dialect }
    }

    pub fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    pub fn name(&self) -> &'static str {
        "sql"
    }

    pub fn lower_with_details(
        &self,
        path: &RepoPath,
        source: &str,
    ) -> Result<SqlLowerOutput, LowerError> {
        if path.extension() != Some("sql") {
            return Err(LowerError::UnsupportedFile {
                message: format!("{} is not a SQL source file", path.as_str()),
            });
        }
        if source_set::has_template_markers(source, self.dialect) {
            return Err(LowerError::Unparsable {
                message: "templated SQL".to_owned(),
            });
        }

        let tokenized = tokenize::tokenize_source(source, self.dialect)?;
        let extracted = statements::extract_statements(&tokenized.chunks);
        let (code_lines, duplication_tokens) =
            collect_lines_and_tokens(&extracted, &tokenized.command_lines);
        let source_lines = u32::try_from(code_lines.len()).map_err(|_| LowerError::TooLarge)?;

        let functions = build_functions(&extracted, &code_lines);
        let (parsed_statements, rejected_statements) =
            parse_all_statements(path, self.dialect, extracted);
        let flagged_lines = flagged::evaluate(&parsed_statements, &code_lines);
        let hazard_lines = hazards::collect_statement_hazards(&parsed_statements, self.dialect);
        let source_set = source_set::classify(path, source);

        Ok(SqlLowerOutput {
            file_model: FileModel {
                runtime_imports: Vec::new(),
                flagged_lines,
                hazard_lines,
                tokens: duplication_tokens,
                functions,
                source_lines,
                source_set,
                program_start: false,
            },
            rejected_statements,
            parsed_statements,
        })
    }
}

impl LanguagePlugin for Sql {
    fn extensions(&self) -> Vec<String> {
        vec!["sql".to_owned()]
    }

    fn lower(&self, path: &RepoPath, text: &str) -> Result<FileModel, LowerError> {
        self.lower_with_details(path, text)
            .map(|output| output.file_model)
    }

    fn has_imports(&self) -> bool {
        false
    }

    fn manifest_paths(
        &self,
        _repository_paths: &[RepoPath],
    ) -> Result<Vec<String>, LanguagePluginError> {
        Ok(Vec::new())
    }

    fn has_hazard_rules(&self) -> bool {
        true
    }

    fn workspace_packages(
        &self,
        _manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, WorkspaceError> {
        Ok(Vec::new())
    }

    fn hazard_rules(&self) -> Vec<String> {
        SQL_HAZARD_RULES
            .iter()
            .map(|rule| (*rule).to_owned())
            .collect()
    }

    fn resolve_imports(
        &self,
        _repository_paths: &[RepoPath],
        _manifests: Vec<Manifest>,
        _requests: Vec<ImportRequest>,
        _active_profiles: &[String],
    ) -> Result<Vec<Resolution>, LanguagePluginError> {
        Ok(Vec::new())
    }

    fn reference_ranges(&self) -> Result<ReferenceRanges, LanguagePluginError> {
        ReferenceRanges::new(
            codeint_model::scan::CoreReferenceRanges {
                erosion: (0.000, 0.918),
                cognitive_complexity: (0.000, 0.902),
                verbosity: (0.000, 0.000),
                duplication: (0.000, 0.909),
                import_cycles: None,
            }
            .build("codeint stratified panel of 77 SQL repositories, ADR 0035, codeint 0.8.0"),
        )
        .map_err(LanguagePluginError::InvalidReferenceRange)
    }

    fn declared_dependencies(
        &self,
        _manifests: Vec<Manifest>,
    ) -> Result<Vec<DeclaredDep>, LanguagePluginError> {
        Ok(Vec::new())
    }

    codeint_model::empty_framework_profile_plugin_methods!();

    fn entry_points(
        &self,
        _repository_paths: &[RepoPath],
        _manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, LanguagePluginError> {
        Ok(Vec::new())
    }
}

fn collect_lines_and_tokens(
    extracted: &[statements::ExtractedStatement],
    command_lines: &BTreeSet<u32>,
) -> (BTreeSet<u32>, Vec<Token>) {
    let mut code_lines = command_lines.clone();
    let mut tokens = Vec::new();
    for stmt in extracted {
        tokenize::collect_statement_lines_and_tokens(
            &stmt.tokens,
            &stmt.routine_body_tokens,
            &mut code_lines,
            &mut tokens,
        );
    }
    (code_lines, tokens)
}

fn build_functions(
    extracted: &[statements::ExtractedStatement],
    code_lines: &BTreeSet<u32>,
) -> Vec<Function> {
    extracted
        .iter()
        .map(|stmt| Function {
            line: stmt.start_line,
            end_line: stmt.end_line,
            decision_points: stmt.decision_points,
            source_lines: code_lines.range(stmt.start_line..=stmt.end_line).count() as u32,
            name: stmt.name.clone(),
            cognitive_complexity: stmt.cognitive_complexity,
            coverage: None,
        })
        .collect()
}

fn parse_all_statements(
    path: &RepoPath,
    dialect: SqlDialect,
    extracted: Vec<statements::ExtractedStatement>,
) -> (Vec<ParsedSqlStatement>, Vec<RejectedSqlStatement>) {
    let mut parsed = Vec::new();
    let mut rejected = Vec::new();
    for stmt in extracted {
        match parser::parse_single_statement(dialect, stmt.tokens.clone()) {
            Some(statement) => parsed.push(ParsedSqlStatement {
                start_line: stmt.start_line,
                end_line: stmt.end_line,
                statement,
                tokens: stmt.tokens,
            }),
            None => rejected.push(RejectedSqlStatement {
                path: path.clone(),
                line: stmt.start_line,
                dialect,
            }),
        }
    }
    (parsed, rejected)
}
