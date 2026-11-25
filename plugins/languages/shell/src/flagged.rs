use std::collections::BTreeSet;

use codeint_model::language::FlaggedLine;
use codeint_model::scan::{finalize_flagged, flag_range};

use crate::commands::{SimpleCommand, parse_commands};
use crate::lexer::{ShellToken, ShellTokenKind};

pub const SHELL_FLAGGED_RULES: &[&str] = &[
    "useless-cat",
    "legacy-backticks",
    "redundant-exit-status-check",
    "else-after-return",
];

pub fn flagged_line_rules() -> impl Iterator<Item = &'static str> {
    SHELL_FLAGGED_RULES.iter().copied()
}

pub(crate) fn evaluate(tokens: &[ShellToken], code_lines: &BTreeSet<u32>) -> Vec<FlaggedLine> {
    let mut out = BTreeSet::new();
    evaluate_stream(tokens, code_lines, &mut out);
    finalize_flagged(out)
}

fn evaluate_stream(
    tokens: &[ShellToken],
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    check_backticks(tokens, code_lines, out);
    check_commands(tokens, code_lines, out);
    check_else_after_return(tokens, code_lines, out);
    for tok in tokens {
        for sub in &tok.sub_tokens {
            evaluate_stream(sub, code_lines, out);
        }
    }
}

fn check_backticks(
    tokens: &[ShellToken],
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    for tok in tokens {
        if tok.has_backtick {
            for &line in &tok.backtick_lines {
                if code_lines.contains(&line) {
                    out.insert((line, "legacy-backticks"));
                }
            }
        }
    }
}

fn check_commands(
    tokens: &[ShellToken],
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    let parsed = parse_commands(tokens);
    for cmd in &parsed.commands {
        check_useless_cat(tokens, cmd, code_lines, out);
        check_redundant_exit_status(tokens, cmd, code_lines, out);
    }
    check_double_bracket_exit_status(tokens, &parsed.double_bracket_indices, code_lines, out);
}

fn check_useless_cat(
    tokens: &[ShellToken],
    cmd: &SimpleCommand,
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    let Some(cmd_idx) = cmd.cmd_idx else {
        return;
    };
    let cmd_tok = &tokens[cmd_idx];
    if !cmd_tok.is_word("cat")
        || cmd.preceded_by_pipe
        || cmd.has_redirection
        || cmd.arg_indices.len() != 1
        || !matches!(cmd.followed_by_op.as_deref(), Some("|" | "|&"))
    {
        return;
    }
    let arg = &tokens[cmd.arg_indices[0]];
    if is_single_file_cat_arg(arg) {
        flag_range(cmd_tok.line, arg.end_line, "useless-cat", code_lines, out);
    }
}

fn is_single_file_cat_arg(arg: &ShellToken) -> bool {
    if arg.text == "-" || arg.text.starts_with('-') {
        return false;
    }
    if !arg.has_quotes && arg.text.bytes().any(|b| matches!(b, b'*' | b'?' | b'[')) {
        return false;
    }
    true
}

fn check_redundant_exit_status(
    tokens: &[ShellToken],
    cmd: &SimpleCommand,
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    let Some(cmd_idx) = cmd.cmd_idx else {
        return;
    };
    let cmd_tok = &tokens[cmd_idx];
    if !(cmd_tok.is_word("[") || cmd_tok.is_word("test")) {
        return;
    }
    let args: Vec<&ShellToken> = cmd.arg_indices.iter().map(|&i| &tokens[i]).collect();
    if contains_redundant_status_triple(&args) && code_lines.contains(&cmd_tok.line) {
        out.insert((cmd_tok.line, "redundant-exit-status-check"));
    }
}

fn check_double_bracket_exit_status(
    tokens: &[ShellToken],
    indices: &[usize],
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    let mut start = 0usize;
    while start < indices.len() {
        let open_tok = &tokens[indices[start]];
        if !open_tok.is_word("[[") {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < indices.len() && !tokens[indices[end]].is_word("]]") {
            end += 1;
        }
        let inner: Vec<&ShellToken> = indices[start + 1..end]
            .iter()
            .map(|&i| &tokens[i])
            .collect();
        if contains_redundant_status_triple(&inner) && code_lines.contains(&open_tok.line) {
            out.insert((open_tok.line, "redundant-exit-status-check"));
        }
        start = end + 1;
    }
}

fn contains_redundant_status_triple(args: &[&ShellToken]) -> bool {
    args.windows(3).any(|win| {
        is_exit_status_cmp(&win[1].text)
            && ((is_exit_status_token(&win[0].text) && is_zero_literal(&win[2].text))
                || (is_zero_literal(&win[0].text) && is_exit_status_token(&win[2].text)))
    })
}

fn is_exit_status_token(text: &str) -> bool {
    matches!(text, "$?" | "\"$?\"" | "${?}" | "\"${?}\"")
}

fn is_zero_literal(text: &str) -> bool {
    matches!(text, "0" | "\"0\"" | "'0'")
}

fn is_exit_status_cmp(text: &str) -> bool {
    matches!(text, "-eq" | "-ne" | "=" | "==" | "!=")
}

struct IfFrame {
    last_then_start: Option<usize>,
    else_idx: Option<usize>,
}

fn check_else_after_return(
    tokens: &[ShellToken],
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    let mut stack: Vec<IfFrame> = Vec::new();
    let mut at_cmd_pos = true;
    for (idx, tok) in tokens.iter().enumerate() {
        if at_cmd_pos && !tok.has_quotes {
            step_if_frame(tokens, idx, &tok.text, &mut stack, code_lines, out);
        }
        at_cmd_pos = tok.next_is_cmd_pos();
    }
}

fn step_if_frame(
    tokens: &[ShellToken],
    idx: usize,
    word: &str,
    stack: &mut Vec<IfFrame>,
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    match word {
        "if" => stack.push(IfFrame {
            last_then_start: None,
            else_idx: None,
        }),
        "then" => set_last_then(stack, Some(idx + 1)),
        "elif" => set_last_then(stack, None),
        "else" => mark_else_after_return(tokens, idx, stack),
        "fi" => close_if_frame(tokens, idx, stack, code_lines, out),
        _ => {}
    }
}

fn set_last_then(stack: &mut [IfFrame], value: Option<usize>) {
    if let Some(frame) = stack.last_mut() {
        frame.last_then_start = value;
    }
}

fn mark_else_after_return(tokens: &[ShellToken], idx: usize, stack: &mut [IfFrame]) {
    if let Some(frame) = stack.last_mut()
        && let Some(then_start) = frame.last_then_start
        && ends_with_unconditional_exit(&tokens[then_start..idx])
    {
        frame.else_idx = Some(idx);
    }
}

fn close_if_frame(
    tokens: &[ShellToken],
    idx: usize,
    stack: &mut Vec<IfFrame>,
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    if let Some(frame) = stack.pop()
        && let Some(else_idx) = frame.else_idx
    {
        flag_range(
            tokens[else_idx].line,
            tokens[idx].line,
            "else-after-return",
            code_lines,
            out,
        );
    }
}

struct ExitScan {
    depth: usize,
    last_stmt_start: Option<usize>,
    last_stmt_conditional: bool,
    after_list_op: bool,
    at_cmd_pos: bool,
}

impl ExitScan {
    fn new() -> Self {
        Self {
            depth: 0,
            last_stmt_start: None,
            last_stmt_conditional: false,
            after_list_op: false,
            at_cmd_pos: true,
        }
    }

    fn step(&mut self, idx: usize, tok: &ShellToken) {
        if self.depth == 0 && tok.is_newline() {
            self.at_cmd_pos = true;
            self.after_list_op = false;
            return;
        }
        if self.depth == 0 && tok.kind == ShellTokenKind::Operator {
            self.step_operator(tok);
            return;
        }
        self.step_word(idx, tok);
    }

    fn step_operator(&mut self, tok: &ShellToken) {
        if matches!(tok.text.as_str(), "&&" | "||" | "|" | "|&") {
            self.last_stmt_conditional = true;
            self.after_list_op = true;
        } else if matches!(tok.text.as_str(), ";" | "&") {
            self.after_list_op = false;
        }
        self.at_cmd_pos = tok.next_is_cmd_pos();
    }

    fn step_word(&mut self, idx: usize, tok: &ShellToken) {
        if self.at_cmd_pos && !tok.has_quotes {
            let prev_depth = self.depth;
            update_block_depth(&tok.text, &mut self.depth);
            if prev_depth == 0 {
                self.last_stmt_start = Some(idx);
                self.last_stmt_conditional = self.after_list_op;
            }
        }
        self.at_cmd_pos = tok.next_is_cmd_pos();
    }
}

fn ends_with_unconditional_exit(slice: &[ShellToken]) -> bool {
    let mut scan = ExitScan::new();
    for (idx, tok) in slice.iter().enumerate() {
        scan.step(idx, tok);
    }
    !scan.last_stmt_conditional
        && scan
            .last_stmt_start
            .is_some_and(|i| slice[i].is_word("return") || slice[i].is_word("exit"))
}

fn update_block_depth(word: &str, depth: &mut usize) {
    match word {
        "if" | "for" | "while" | "until" | "select" | "case" | "{" => *depth += 1,
        "fi" | "done" | "esac" | "}" => *depth = depth.saturating_sub(1),
        _ => {}
    }
}
