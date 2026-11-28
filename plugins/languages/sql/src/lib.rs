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
    FileDiagnostic, FileModel, FileScopeArtifact, Function, ScopeAnalysis,
};
use codeint_model::scan::CoreReferenceRanges;
use codeint_model::{
    LanguagePlugin, LanguagePluginError, LowerError, ReferenceRanges, RepoPath, Token,
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
    const ALL: [SqlDialect; 5] = [
        SqlDialect::Generic,
        SqlDialect::Postgresql,
        SqlDialect::Mysql,
        SqlDialect::Sqlite,
        SqlDialect::Duckdb,
    ];

    pub(crate) fn from_name(name: &str) -> Option<SqlDialect> {
        SqlDialect::ALL
            .into_iter()
            .find(|dialect| dialect.as_str() == name)
    }

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
    pub const fn new() -> Sql {
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
        let diagnostics = rejected_statements
            .iter()
            .map(|entry| FileDiagnostic {
                line: entry.line,
                kind: "rejected-statement".to_owned(),
                message: format!(
                    "could not parse statement as {}",
                    entry.dialect.display_name()
                ),
            })
            .collect();
        let scope_artifact = Some(schema::encode_file_artifact(
            path,
            &parsed_statements,
            self.dialect,
        ));

        Ok(SqlLowerOutput {
            file_model: FileModel {
                flagged_lines,
                hazard_lines,
                tokens: duplication_tokens,
                functions,
                source_lines,
                diagnostics,
                scope_artifact,
                ..codeint_model::empty_file_model(source_set)
            },
            rejected_statements,
            parsed_statements,
        })
    }
}

const SQL_RANGES: CoreReferenceRanges = CoreReferenceRanges::new(
    (0.000, 0.918),
    (0.000, 0.902),
    (0.000, 0.000),
    (0.000, 0.909),
    None,
);

impl LanguagePlugin for Sql {
    fn extensions(&self) -> Vec<String> {
        vec!["sql".to_owned()]
    }

    fn known_dialects(&self) -> Vec<String> {
        vec![
            "postgresql".to_owned(),
            "mysql".to_owned(),
            "sqlite".to_owned(),
            "duckdb".to_owned(),
            "generic".to_owned(),
        ]
    }

    fn lower(
        &self,
        path: &RepoPath,
        text: &str,
        dialect: Option<&str>,
    ) -> Result<FileModel, LowerError> {
        let selected = match dialect {
            Some(name) => {
                SqlDialect::from_name(name).ok_or_else(|| LowerError::UnsupportedFile {
                    message: format!("unknown SQL dialect {name:?}"),
                })?
            }
            None => self.dialect,
        };
        Sql::with_dialect(selected)
            .lower_with_details(path, text)
            .map(|output| output.file_model)
    }

    fn flagged_line_rules(&self) -> Vec<String> {
        flagged_line_rules().map(str::to_owned).collect()
    }

    fn reference_ranges(&self) -> Result<ReferenceRanges, LanguagePluginError> {
        ReferenceRanges::new(
            SQL_RANGES
                .build("codeint stratified panel of 77 SQL repositories, ADR 0035, codeint 0.8.0"),
        )
        .map_err(LanguagePluginError::InvalidReferenceRange)
    }

    fn has_hazard_rules(&self) -> bool {
        true
    }

    fn hazard_rules(&self) -> Vec<String> {
        SQL_HAZARD_RULES
            .iter()
            .map(|rule| (*rule).to_owned())
            .collect()
    }

    fn has_scope_analysis(&self) -> bool {
        true
    }

    fn analyze_scope(
        &self,
        _package_root: &str,
        dialect: Option<&str>,
        artifacts: Vec<FileScopeArtifact>,
    ) -> Result<ScopeAnalysis, LanguagePluginError> {
        Ok(schema::analyze_scope_artifacts(dialect, artifacts))
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
