use oxc_ast::ast::{FunctionBody, Program, Statement};
use oxc_ast_visit::{Visit, walk};
use oxc_span::Span;

use super::Rule;

pub(super) const RULE: Rule = Rule {
    id: "trailing-bare-return",
    find: find_trailing_bare_returns,
};

fn find_trailing_bare_returns(program: &Program<'_>, _code: &str) -> Vec<Span> {
    let mut finder = TrailingBareReturnFinder { spans: Vec::new() };
    finder.visit_program(program);
    finder.spans
}

struct TrailingBareReturnFinder {
    spans: Vec<Span>,
}

impl<'a> Visit<'a> for TrailingBareReturnFinder {
    fn visit_function_body(&mut self, body: &FunctionBody<'a>) {
        if let Some(Statement::ReturnStatement(last)) = body.statements.last()
            && last.argument.is_none()
        {
            self.spans.push(last.span);
        }
        walk::walk_function_body(self, body);
    }
}
