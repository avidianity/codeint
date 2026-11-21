use oxc_ast::ast::{ConditionalExpression, Expression, Program};
use oxc_ast_visit::{Visit, walk};
use oxc_span::{GetSpan, Span};

use super::Rule;

pub(super) const RULE: Rule = Rule {
    id: "conditional-yields-boolean",
    find: find_conditionals_yielding_opposite_booleans,
};

fn find_conditionals_yielding_opposite_booleans(program: &Program<'_>, _code: &str) -> Vec<Span> {
    let mut finder = BooleanConditionalFinder { spans: Vec::new() };
    finder.visit_program(program);
    finder.spans
}

struct BooleanConditionalFinder {
    spans: Vec<Span>,
}

impl<'a> Visit<'a> for BooleanConditionalFinder {
    fn visit_conditional_expression(&mut self, conditional: &ConditionalExpression<'a>) {
        if yields_opposite_booleans(conditional) {
            self.spans.push(conditional.span());
        }
        walk::walk_conditional_expression(self, conditional);
    }
}

fn yields_opposite_booleans(conditional: &ConditionalExpression<'_>) -> bool {
    match (&conditional.consequent, &conditional.alternate) {
        (Expression::BooleanLiteral(consequent), Expression::BooleanLiteral(alternate)) => {
            consequent.value != alternate.value
        }
        _ => false,
    }
}
