use crate::commands::parse_commands;
use crate::dialect::{ShellDialect, parse_shebang_flags};
use crate::functions::{FunctionSpan, child_span_end};
use crate::lexer::ShellToken;

pub(super) struct StrictModeInfo {
    pub(super) has_errexit: bool,
    pub(super) has_strict_mode: bool,
    pub(super) has_top_level_code: bool,
}

#[derive(Default)]
struct StrictFlags {
    has_e: bool,
    has_u: bool,
    has_pipefail: bool,
}

pub(super) fn analyze(
    tokens: &[ShellToken],
    spans: &[FunctionSpan],
    dialect: ShellDialect,
    source: &str,
) -> StrictModeInfo {
    let mut flags = StrictFlags::default();
    let shebang_flags = parse_shebang_flags(source);
    let shebang_refs: Vec<&str> = shebang_flags.iter().map(String::as_str).collect();
    apply_set_args(&shebang_refs, &mut flags);

    let top_level = collect_top_level_tokens(tokens, spans);
    let has_top_level_code = top_level.iter().any(|tok| !tok.is_newline());
    let parsed = parse_commands(&top_level);
    for cmd in &parsed.commands {
        let Some(cmd_idx) = cmd.cmd_idx else {
            continue;
        };
        let cmd_tok = &top_level[cmd_idx];
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

    let has_strict_mode = match dialect {
        ShellDialect::PosixSh => flags.has_e && flags.has_u,
        ShellDialect::Bash | ShellDialect::Zsh => flags.has_e && flags.has_u && flags.has_pipefail,
    };
    StrictModeInfo {
        has_errexit: flags.has_e,
        has_strict_mode,
        has_top_level_code,
    }
}

fn collect_top_level_tokens(tokens: &[ShellToken], spans: &[FunctionSpan]) -> Vec<ShellToken> {
    let mut out = Vec::new();
    let mut idx = 0usize;
    while idx < tokens.len() {
        if let Some(end) = child_span_end(spans, None, idx) {
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
