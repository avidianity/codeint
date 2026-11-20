use oxc_ast::ast::Program;
use oxc_span::Span;

use super::Rule;

pub(super) const RULE: Rule = Rule {
    id: "else-after-return",
    find: find_else_keywords,
};

fn find_else_keywords(program: &Program<'_>, code: &str) -> Vec<Span> {
    todo!("{program:?} {code}")
}
