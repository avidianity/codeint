use codeint_model::RepoPath;
use sqlparser::ast::Statement;
use sqlparser::parser::Parser;
use sqlparser::tokenizer::{Token, TokenWithSpan};

use crate::SqlDialect;
use crate::tokenize::sqlparser_dialect;

const MAX_PARSED_STATEMENT_TOKENS: usize = 12_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RejectedSqlStatement {
    pub path: RepoPath,
    pub line: u32,
    pub dialect: SqlDialect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedSqlStatement {
    pub start_line: u32,
    pub end_line: u32,
    pub statement: Statement,
    pub tokens: Vec<TokenWithSpan>,
}

pub(crate) fn parse_single_statement(
    dialect: SqlDialect,
    tokens: Vec<TokenWithSpan>,
) -> Option<Statement> {
    let code_tokens = tokens
        .iter()
        .filter(|token| !matches!(token.token, Token::Whitespace(_)))
        .count();
    if code_tokens > MAX_PARSED_STATEMENT_TOKENS {
        return None;
    }
    let mut parser = Parser::new(sqlparser_dialect(dialect)).with_tokens_with_locations(tokens);
    let mut statements = parser.parse_statements().ok()?;
    if statements.len() == 1 {
        statements.pop()
    } else {
        None
    }
}
