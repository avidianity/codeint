use codeint_model::language::{LowerError, Token, TokenKind};
use oxc_ast::ast::Program;
use oxc_parser::Kind;

pub(crate) fn tokens(
    text: &str,
    lexed: &[oxc_parser::Token],
    program: &Program<'_>,
) -> Result<Vec<Token>, LowerError> {
    let _ = (text, lexed, identifier_starts(program), kind);
    todo!()
}

fn identifier_starts(program: &Program<'_>) -> Vec<u32> {
    let _ = program;
    todo!()
}

fn kind(lexed: Kind, is_identifier: bool) -> TokenKind {
    let _ = (lexed, is_identifier);
    todo!()
}
