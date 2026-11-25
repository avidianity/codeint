mod bashisms;
mod commands;
mod secrets;
mod strict_mode;

use std::collections::BTreeSet;

use codeint_model::HazardLine;

use crate::dialect::ShellDialect;
use crate::functions::FunctionSpan;
use crate::lexer::ShellToken;

pub const SHELL_HAZARD_RULES: &[&str] = &[
    "missing-strict-mode",
    "unquoted-variable",
    "eval-with-variable",
    "pipe-download-to-shell",
    "unchecked-cd",
    "unguarded-rm-rf",
    "hardcoded-secret",
    "read-without-r",
    "posix-sh-bashism",
    "insecure-tls-download",
];

pub fn hazard_line_rules() -> impl Iterator<Item = &'static str> {
    SHELL_HAZARD_RULES.iter().copied()
}

pub(crate) fn evaluate(
    tokens: &[ShellToken],
    code_lines: &BTreeSet<u32>,
    spans: &[FunctionSpan],
    dialect: ShellDialect,
    source: &str,
) -> Vec<HazardLine> {
    let mut hits: BTreeSet<(u32, &'static str)> = BTreeSet::new();
    let strict = strict_mode::analyze(tokens, spans, dialect, source);
    if strict.has_top_level_code
        && !strict.has_strict_mode
        && let Some(&first_code_line) = code_lines.first()
    {
        hits.insert((first_code_line, "missing-strict-mode"));
    }
    evaluate_stream(tokens, code_lines, strict.has_errexit, dialect, &mut hits);
    hits.into_iter()
        .map(|(line, rule)| HazardLine {
            line,
            rule: rule.to_owned(),
        })
        .collect()
}

fn evaluate_stream(
    tokens: &[ShellToken],
    code_lines: &BTreeSet<u32>,
    has_errexit: bool,
    dialect: ShellDialect,
    hits: &mut BTreeSet<(u32, &'static str)>,
) {
    commands::check_stream(tokens, code_lines, has_errexit, hits);
    secrets::check_stream(tokens, code_lines, hits);
    if dialect == ShellDialect::PosixSh {
        bashisms::check_stream(tokens, code_lines, hits);
    }
    for tok in tokens {
        for sub in &tok.sub_tokens {
            evaluate_stream(sub, code_lines, has_errexit, dialect, hits);
        }
    }
}

pub(super) fn emit_line(
    line: u32,
    rule: &'static str,
    code_lines: &BTreeSet<u32>,
    hits: &mut BTreeSet<(u32, &'static str)>,
) {
    if code_lines.contains(&line) {
        hits.insert((line, rule));
    }
}
