use oxc_ast::ast::{Expression, IfStatement, Program, Statement};
use oxc_ast_visit::{Visit, walk};
use oxc_span::{GetSpan, Span};

use super::Rule;

pub(super) const RULE: Rule = Rule {
    id: "if-else-returns-boolean",
    find: find_if_else_returning_opposite_booleans,
};

fn find_if_else_returning_opposite_booleans(program: &Program<'_>, _code: &str) -> Vec<Span> {
    let mut finder = BooleanIfElseFinder { spans: Vec::new() };
    finder.visit_program(program);
    finder.spans
}

struct BooleanIfElseFinder {
    spans: Vec<Span>,
}

impl<'a> Visit<'a> for BooleanIfElseFinder {
    fn visit_if_statement(&mut self, if_statement: &IfStatement<'a>) {
        if returns_opposite_booleans(if_statement) {
            self.spans.push(if_statement.span());
        }
        walk::walk_if_statement(self, if_statement);
    }
}

fn returns_opposite_booleans(if_statement: &IfStatement<'_>) -> bool {
    let Some(alternate) = &if_statement.alternate else {
        return false;
    };
    match (
        returned_boolean(&if_statement.consequent),
        returned_boolean(alternate),
    ) {
        (Some(consequent), Some(alternate)) => consequent != alternate,
        _ => false,
    }
}

fn returned_boolean(statement: &Statement<'_>) -> Option<bool> {
    match statement {
        Statement::ReturnStatement(return_statement) => match &return_statement.argument {
            Some(Expression::BooleanLiteral(literal)) => Some(literal.value),
            _ => None,
        },
        Statement::BlockStatement(block) => match block.body.as_slice() {
            [only] => returned_boolean(only),
            _ => None,
        },
        _ => None,
    }
}
