use std::collections::BTreeSet;

use codeint_model::language::{FlaggedLine, LowerError};
use oxc_ast::ast::Program;
use oxc_span::Span;

use crate::is_code_character;

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
    let lines = Lines::of(code);
    let mut flagged = BTreeSet::new();
    for rule in &RULES {
        for span in (rule.find)(program, code) {
            for line in lines.source_lines_in(span) {
                flagged.insert((line, rule.id));
            }
        }
    }
    flagged
        .into_iter()
        .map(|(line, rule)| {
            Ok(FlaggedLine {
                line: u32::try_from(line + 1).map_err(|_| LowerError::TooLarge)?,
                rule: rule.to_owned(),
            })
        })
        .collect()
}

struct Lines<'code> {
    code: &'code str,
    starts: Vec<usize>,
}

impl<'code> Lines<'code> {
    fn of(code: &'code str) -> Lines<'code> {
        let starts = std::iter::once(0)
            .chain(code.match_indices('\n').map(|(index, _)| index + 1))
            .collect();
        Lines { code, starts }
    }

    fn source_lines_in(&self, span: Span) -> impl Iterator<Item = usize> + '_ {
        let first = self.index_of(span.start as usize);
        let last = self.index_of(
            (span.end as usize)
                .saturating_sub(1)
                .max(span.start as usize),
        );
        (first..=last).filter(|&index| self.holds_code(index))
    }

    fn index_of(&self, offset: usize) -> usize {
        self.starts.partition_point(|&start| start <= offset) - 1
    }

    fn holds_code(&self, index: usize) -> bool {
        let end = self
            .starts
            .get(index + 1)
            .copied()
            .unwrap_or(self.code.len());
        self.code[self.starts[index]..end]
            .chars()
            .any(is_code_character)
    }
}
