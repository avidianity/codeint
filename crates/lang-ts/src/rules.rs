use std::collections::BTreeSet;

use codeint_model::language::{FlaggedLine, LowerError};
use oxc_ast::ast::Program;
use oxc_span::Span;

use crate::functions::CodeLines;

mod else_after_return;

struct Rule {
    id: &'static str,
    find: fn(&Program<'_>, &str) -> Vec<Span>,
}

const RULES: [Rule; 1] = [else_after_return::RULE];

pub fn flagged_line_rules() -> impl Iterator<Item = &'static str> {
    RULES.iter().map(|rule| rule.id)
}

pub(crate) fn flag(
    program: &Program<'_>,
    code: &str,
    code_lines: &CodeLines,
) -> Result<Vec<FlaggedLine>, LowerError> {
    let mut flagged = BTreeSet::new();
    for rule in &RULES {
        for span in (rule.find)(program, code) {
            for line_index in code_lines.source_line_indexes_in(span) {
                flagged.insert((line_index, rule.id));
            }
        }
    }
    flagged
        .into_iter()
        .map(|(line_index, rule)| {
            Ok(FlaggedLine {
                line: u32::try_from(line_index + 1).map_err(|_| LowerError::TooLarge)?,
                rule: rule.to_owned(),
            })
        })
        .collect()
}
