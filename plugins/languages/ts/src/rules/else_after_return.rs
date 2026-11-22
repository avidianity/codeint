use oxc_allocator::Vec as ArenaVec;
use oxc_ast::ast::{Declaration, IfStatement, Program, Statement, VariableDeclarationKind};
use oxc_ast_visit::{Visit, walk};
use oxc_span::{GetSpan, Span};

use super::Rule;

pub(super) const RULE: Rule = Rule {
    id: "else-after-return",
    find: find_else_keywords,
};

const ELSE: &str = "else";

fn find_else_keywords(program: &Program<'_>, code: &str) -> Vec<Span> {
    let mut finder = ElseKeywordFinder {
        code,
        spans: Vec::new(),
    };
    finder.visit_program(program);
    finder.spans
}

struct ElseKeywordFinder<'code> {
    code: &'code str,
    spans: Vec<Span>,
}

impl<'a> Visit<'a> for ElseKeywordFinder<'_> {
    fn visit_statements(&mut self, statements: &ArenaVec<'a, Statement<'a>>) {
        for statement in statements {
            if let Statement::IfStatement(if_statement) = statement
                && let Some(span) = self.collapsible_else_keyword(if_statement)
            {
                self.spans.push(span);
            }
        }
        walk::walk_statements(self, statements);
    }
}

impl ElseKeywordFinder<'_> {
    fn collapsible_else_keyword(&self, chain: &IfStatement<'_>) -> Option<Span> {
        let mut branch = chain;
        loop {
            if !always_returns(&branch.consequent) {
                return None;
            }
            match branch.alternate.as_ref()? {
                Statement::IfStatement(next) => branch = next,
                alternate if declares_block_scoped_name(alternate) => return None,
                alternate => {
                    return self.else_keyword_between(branch.consequent.span(), alternate.span());
                }
            }
        }
    }

    fn else_keyword_between(&self, consequent: Span, alternate: Span) -> Option<Span> {
        let gap_start = consequent.end as usize;
        let offset = self.code[gap_start..alternate.start as usize].rfind(ELSE)?;
        let start = u32::try_from(gap_start + offset).ok()?;
        Some(Span::sized(start, ELSE.len() as u32))
    }
}

fn always_returns(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::ReturnStatement(_) => true,
        Statement::BlockStatement(block) => block.body.iter().any(always_returns),
        Statement::IfStatement(if_statement) => {
            always_returns(&if_statement.consequent)
                && if_statement.alternate.as_ref().is_some_and(always_returns)
        }
        _ => false,
    }
}

fn declares_block_scoped_name(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::BlockStatement(block) => block.body.iter().any(is_block_scoped_declaration),
        other => is_block_scoped_declaration(other),
    }
}

fn is_block_scoped_declaration(statement: &Statement<'_>) -> bool {
    match statement.as_declaration() {
        Some(Declaration::VariableDeclaration(variables)) => {
            variables.kind != VariableDeclarationKind::Var
        }
        Some(_) => true,
        None => false,
    }
}
