use std::collections::BTreeSet;

use crate::commands::parse_commands;
use crate::dialect::{ShellDialect, parse_shebang_flags};
use crate::functions::{ExtractedFunctions, FunctionSpan, SpanIndex};
use crate::lexer::{ShellToken, ShellTokenKind};

pub(super) struct StrictModeInfo {
    pub(super) has_errexit: bool,
    pub(super) has_strict_mode: bool,
    pub(super) has_top_level_code: bool,
    pub(super) errexit_tokens: Vec<bool>,
}

#[derive(Default)]
struct StrictFlags {
    has_e: bool,
    has_u: bool,
    has_pipefail: bool,
}

impl StrictFlags {
    fn merge(&mut self, other: &Self) {
        self.has_e |= other.has_e;
        self.has_u |= other.has_u;
        self.has_pipefail |= other.has_pipefail;
    }
}

pub(super) fn analyze(
    tokens: &[ShellToken],
    extracted: &ExtractedFunctions,
    dialect: ShellDialect,
    source: &str,
) -> StrictModeInfo {
    let mut flags = StrictFlags::default();
    let shebang_flags = parse_shebang_flags(source);
    let shebang_refs: Vec<&str> = shebang_flags.iter().map(String::as_str).collect();
    apply_set_args(&shebang_refs, &mut flags);

    let top_level = collect_top_level_tokens(tokens, &extracted.index);
    let has_top_level_code = top_level.iter().any(|tok| !tok.is_newline());
    let parsed = parse_commands(&top_level);
    let mut called: BTreeSet<&str> = BTreeSet::new();
    for cmd in &parsed.commands {
        let Some(cmd_idx) = cmd.cmd_idx else {
            continue;
        };
        let cmd_tok = &top_level[cmd_idx];
        if !cmd_tok.has_quotes {
            called.insert(cmd_tok.text.as_str());
        }
        let args: Vec<&str> = cmd
            .arg_indices
            .iter()
            .map(|&i| top_level[i].text.as_str())
            .collect();
        if cmd_tok.is_word("set") {
            apply_set_args(&args, &mut flags);
        } else if dialect == ShellDialect::Zsh && cmd_tok.is_word("setopt") {
            apply_zsh_setopt(&args, &mut flags);
        }
    }

    let top_level_errexit = flags.has_e;
    let function_flags: Vec<(&FunctionSpan, StrictFlags)> = extracted
        .spans
        .iter()
        .filter_map(|span| leading_set_flags(tokens, span, dialect).map(|flags| (span, flags)))
        .collect();
    for (span, fn_flags) in &function_flags {
        if called.contains(span.name.as_str()) {
            flags.merge(fn_flags);
        }
    }
    let errexit_tokens = errexit_token_map(tokens.len(), &function_flags);

    let has_strict_mode = match dialect {
        ShellDialect::PosixSh => flags.has_e && flags.has_u,
        ShellDialect::Bash | ShellDialect::Zsh => flags.has_e && flags.has_u && flags.has_pipefail,
    };
    StrictModeInfo {
        has_errexit: top_level_errexit,
        has_strict_mode,
        has_top_level_code,
        errexit_tokens,
    }
}

fn leading_set_flags(
    tokens: &[ShellToken],
    span: &FunctionSpan,
    dialect: ShellDialect,
) -> Option<StrictFlags> {
    let opener = tokens.get(span.body_start)?;
    if !(opener.is_word("{") || opener.is_op("(")) {
        return None;
    }
    let mut flags = StrictFlags::default();
    let mut idx = span.body_start + 1;
    while idx < span.body_end {
        if tokens[idx].is_newline() || tokens[idx].is_op(";") {
            idx += 1;
            continue;
        }
        let statement_end = statement_end(tokens, idx, span.body_end);
        let statement = &tokens[idx..statement_end];
        if !apply_leading_statement(statement, dialect, &mut flags) {
            break;
        }
        idx = statement_end;
    }
    (flags.has_e || flags.has_u || flags.has_pipefail).then_some(flags)
}

fn statement_end(tokens: &[ShellToken], start: usize, limit: usize) -> usize {
    let mut idx = start;
    while idx < limit && !tokens[idx].is_newline() && !tokens[idx].is_op(";") {
        idx += 1;
    }
    idx
}

fn apply_leading_statement(
    statement: &[ShellToken],
    dialect: ShellDialect,
    flags: &mut StrictFlags,
) -> bool {
    let Some((head, rest)) = statement.split_first() else {
        return false;
    };
    let is_set = head.is_word("set");
    let is_setopt = dialect == ShellDialect::Zsh && head.is_word("setopt");
    if !(is_set || is_setopt) || rest.iter().any(|tok| tok.kind != ShellTokenKind::Word) {
        return false;
    }
    let args: Vec<&str> = rest.iter().map(|tok| tok.text.as_str()).collect();
    if is_set {
        apply_set_args(&args, flags);
    } else {
        apply_zsh_setopt(&args, flags);
    }
    true
}

fn errexit_token_map(
    token_count: usize,
    function_flags: &[(&FunctionSpan, StrictFlags)],
) -> Vec<bool> {
    let mut starts = vec![0i64; token_count + 1];
    for (span, flags) in function_flags {
        if flags.has_e {
            starts[span.body_start] += 1;
            starts[(span.body_end + 1).min(token_count)] -= 1;
        }
    }
    let mut depth = 0i64;
    starts
        .iter()
        .take(token_count)
        .map(|delta| {
            depth += delta;
            depth > 0
        })
        .collect()
}

fn collect_top_level_tokens(tokens: &[ShellToken], index: &SpanIndex) -> Vec<ShellToken> {
    let mut out = Vec::new();
    let mut idx = 0usize;
    while idx < tokens.len() {
        if let Some(end) = index.skip_end(idx) {
            idx = end + 1;
            continue;
        }
        out.push(tokens[idx].clone());
        idx += 1;
    }
    out
}

fn apply_set_args(args: &[&str], flags: &mut StrictFlags) {
    let mut idx = 0usize;
    while idx < args.len() {
        let arg = args[idx];
        if arg == "--" {
            break;
        }
        if let Some(short) = arg.strip_prefix('-')
            && !short.starts_with('-')
        {
            idx += apply_short_set_flag(short, args.get(idx + 1).copied(), flags);
            continue;
        }
        idx += 1;
    }
}

fn apply_short_set_flag(short: &str, next: Option<&str>, flags: &mut StrictFlags) -> usize {
    if short.contains('e') {
        flags.has_e = true;
    }
    if short.contains('u') {
        flags.has_u = true;
    }
    if short.ends_with('o')
        && let Some(opt_name) = next
    {
        apply_named_set_option(opt_name, flags);
        return 2;
    }
    1
}

fn apply_named_set_option(opt: &str, flags: &mut StrictFlags) {
    match opt {
        "errexit" => flags.has_e = true,
        "nounset" => flags.has_u = true,
        "pipefail" => flags.has_pipefail = true,
        _ => {}
    }
}

fn apply_zsh_setopt(args: &[&str], flags: &mut StrictFlags) {
    for &arg in args {
        let normalized: String = arg
            .chars()
            .filter(|&c| c != '_')
            .map(|c| c.to_ascii_lowercase())
            .collect();
        apply_named_set_option(&normalized, flags);
    }
}
