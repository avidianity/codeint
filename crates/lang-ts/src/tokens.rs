use codeint_model::language::{LowerError, Token, TokenKind};
use codeint_model::token_text_hash;
use oxc_ast::ast::{
    BindingIdentifier, IdentifierName, IdentifierReference, JSXIdentifier, LabelIdentifier,
    PrivateIdentifier, Program,
};
use oxc_ast_visit::Visit;
use oxc_parser::Kind;
use oxc_span::Span;

pub(crate) fn tokens(
    text: &str,
    lexed: &[oxc_parser::Token],
    program: &Program<'_>,
) -> Result<Vec<Token>, LowerError> {
    let identifier_starts = identifier_starts(program);
    let mut tokens = Vec::with_capacity(lexed.len());
    let mut line: u32 = 1;
    let mut cursor = 0;
    for token in lexed.iter().filter(|token| is_code_token(token.kind())) {
        let start = token.start() as usize;
        let newlines = text.as_bytes()[cursor..start]
            .iter()
            .filter(|byte| **byte == b'\n')
            .count();
        line = u32::try_from(newlines)
            .ok()
            .and_then(|newlines| line.checked_add(newlines))
            .ok_or(LowerError::TooLarge)?;
        cursor = start;
        let names_an_identifier = identifier_starts.binary_search(&token.start()).is_ok();
        tokens.push(Token {
            kind: kind(token.kind(), names_an_identifier),
            text_hash: token_text_hash(&text[start..token.end() as usize]),
            line,
        });
    }
    Ok(tokens)
}

pub(crate) fn is_code_token(kind: Kind) -> bool {
    !matches!(kind, Kind::Eof | Kind::Skip | Kind::HashbangComment)
}

fn identifier_starts(program: &Program<'_>) -> Vec<u32> {
    let mut collector = IdentifierStarts::default();
    collector.visit_program(program);
    let mut starts = collector.starts;
    starts.sort_unstable();
    starts.dedup();
    starts
}

fn kind(lexed: Kind, names_an_identifier: bool) -> TokenKind {
    if names_an_identifier || matches!(lexed, Kind::Ident | Kind::PrivateIdentifier) {
        TokenKind::Identifier
    } else if lexed.is_literal()
        || matches!(
            lexed,
            Kind::NoSubstitutionTemplate
                | Kind::TemplateHead
                | Kind::TemplateMiddle
                | Kind::TemplateTail
                | Kind::JSXText
        )
    {
        TokenKind::Literal
    } else {
        TokenKind::Other
    }
}

#[derive(Default)]
struct IdentifierStarts {
    starts: Vec<u32>,
}

impl IdentifierStarts {
    fn record(&mut self, span: Span) {
        self.starts.push(span.start);
    }
}

impl<'a> Visit<'a> for IdentifierStarts {
    fn visit_identifier_name(&mut self, it: &IdentifierName<'a>) {
        self.record(it.span);
    }

    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
        self.record(it.span);
    }

    fn visit_binding_identifier(&mut self, it: &BindingIdentifier<'a>) {
        self.record(it.span);
    }

    fn visit_label_identifier(&mut self, it: &LabelIdentifier<'a>) {
        self.record(it.span);
    }

    fn visit_private_identifier(&mut self, it: &PrivateIdentifier<'a>) {
        self.record(it.span);
    }

    fn visit_jsx_identifier(&mut self, it: &JSXIdentifier<'a>) {
        self.record(it.span);
    }
}
