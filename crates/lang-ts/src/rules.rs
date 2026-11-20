use std::collections::BTreeSet;

use codeint_model::language::{FlaggedLine, LowerError};
use oxc_ast::ast::Program;
use oxc_span::Span;

use crate::functions::CodeLines;

mod catch_only_rethrows;
mod conditional_yields_boolean;
mod constructor_only_forwards;
mod else_after_return;
mod if_else_returns_boolean;
mod trailing_bare_return;

struct Rule {
    id: &'static str,
    find: fn(&Program<'_>, &str) -> Vec<Span>,
}

const RULES: [Rule; 6] = [
    else_after_return::RULE,
    if_else_returns_boolean::RULE,
    catch_only_rethrows::RULE,
    conditional_yields_boolean::RULE,
    trailing_bare_return::RULE,
    constructor_only_forwards::RULE,
];

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
