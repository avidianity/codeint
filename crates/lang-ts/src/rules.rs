use codeint_model::language::{FlaggedLine, LowerError};
use oxc_ast::ast::Program;
use oxc_span::Span;

mod else_after_return;

struct Rule {
    id: &'static str,
    find: fn(&Program<'_>, &str) -> Vec<Span>,
}

const RULES: [Rule; 1] = [else_after_return::RULE];

pub fn flagged_line_rules() -> impl Iterator<Item = &'static str> {
    RULES.iter().map(|rule| rule.id)
}

pub(crate) fn flag(program: &Program<'_>, code: &str) -> Result<Vec<FlaggedLine>, LowerError> {
    let spans_by_rule = RULES.map(|rule| (rule.id, (rule.find)(program, code)));
    todo!("{spans_by_rule:?}")
}
