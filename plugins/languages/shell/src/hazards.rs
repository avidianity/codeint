mod bashisms;
mod commands;
mod secrets;
mod strict_mode;

use std::collections::BTreeSet;

use codeint_model::HazardLine;

use crate::dialect::ShellDialect;
use crate::functions::ExtractedFunctions;
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
    extracted: &ExtractedFunctions,
    dialect: ShellDialect,
    source: &str,
) -> Vec<HazardLine> {
    let mut hits: BTreeSet<(u32, &'static str)> = BTreeSet::new();
    let strict = strict_mode::analyze(tokens, extracted, dialect, source);
    if strict.has_top_level_code
        && !strict.has_strict_mode
        && let Some(&first_code_line) = code_lines.first()
    {
        hits.insert((first_code_line, "missing-strict-mode"));
    }
    let scope = if strict.has_errexit {
        ErrexitScope::Uniform(true)
    } else {
        ErrexitScope::PerToken(&strict.errexit_tokens)
    };
    evaluate_stream(tokens, code_lines, scope, dialect, &mut hits);
    hits.into_iter()
        .map(|(line, rule)| HazardLine {
            line,
            rule: rule.to_owned(),
        })
        .collect()
}

#[derive(Clone, Copy)]
pub(super) enum ErrexitScope<'a> {
    Uniform(bool),
    PerToken(&'a [bool]),
}

impl ErrexitScope<'_> {
    pub(super) fn at(self, idx: usize) -> bool {
        match self {
            Self::Uniform(value) => value,
            Self::PerToken(flags) => flags.get(idx).copied().unwrap_or(false),
        }
    }
}

fn evaluate_stream(
    tokens: &[ShellToken],
    code_lines: &BTreeSet<u32>,
    scope: ErrexitScope<'_>,
    dialect: ShellDialect,
    hits: &mut BTreeSet<(u32, &'static str)>,
) {
    commands::check_stream(tokens, code_lines, scope, dialect, hits);
    secrets::check_stream(tokens, code_lines, hits);
    if dialect == ShellDialect::PosixSh {
        bashisms::check_stream(tokens, code_lines, hits);
    }
    for (idx, tok) in tokens.iter().enumerate() {
        for sub in &tok.sub_tokens {
            let sub_scope = ErrexitScope::Uniform(scope.at(idx));
            evaluate_stream(sub, code_lines, sub_scope, dialect, hits);
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
