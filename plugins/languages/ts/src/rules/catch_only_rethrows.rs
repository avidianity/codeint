use oxc_ast::ast::{BindingPattern, CatchClause, Expression, Program, Statement, TryStatement};
use oxc_ast_visit::{Visit, walk};
use oxc_span::Span;

use super::Rule;

pub(super) const RULE: Rule = Rule {
    id: "catch-only-rethrows",
    find: find_rethrowing_catch_clauses,
};

fn find_rethrowing_catch_clauses(program: &Program<'_>, _code: &str) -> Vec<Span> {
    let mut finder = RethrowingCatchFinder { spans: Vec::new() };
    finder.visit_program(program);
    finder.spans
}

struct RethrowingCatchFinder {
    spans: Vec<Span>,
}

impl<'a> Visit<'a> for RethrowingCatchFinder {
    fn visit_try_statement(&mut self, statement: &TryStatement<'a>) {
        if statement.finalizer.is_none()
            && let Some(handler) = &statement.handler
            && only_rethrows(handler)
        {
            self.spans.push(handler.span);
        }
        walk::walk_try_statement(self, statement);
    }
}

fn only_rethrows(handler: &CatchClause<'_>) -> bool {
    let Some(BindingPattern::BindingIdentifier(caught)) =
        handler.param.as_ref().map(|param| &param.pattern)
    else {
        return false;
    };
    let [Statement::ThrowStatement(throw)] = handler.body.body.as_slice() else {
        return false;
    };
    matches!(
        throw.argument.without_parentheses(),
        Expression::Identifier(thrown) if thrown.name == caught.name
    )
}
