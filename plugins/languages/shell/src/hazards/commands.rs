use std::collections::BTreeSet;

use codeint_model::path_file_name;

use crate::commands::{SimpleCommand, parse_commands};
use crate::dialect::ShellDialect;
use crate::lexer::{ShellToken, is_assignment_word};

use super::{ErrexitScope, emit_line};

const ASSIGNMENT_BUILTINS: &[&str] = &["declare", "env", "export", "local", "readonly", "typeset"];

const SHELL_INTERPRETERS: &[&str] = &["ash", "bash", "dash", "ksh", "sh", "zsh"];

const CURL_VALUE_SHORT_FLAGS: &[u8] = b"AbcCdDeEFHKmoTuUwxXyYz";

const READ_VALUE_SHORT_FLAGS: &[u8] = b"adinNptu";

pub(super) fn check_stream(
    tokens: &[ShellToken],
    code_lines: &BTreeSet<u32>,
    scope: ErrexitScope<'_>,
    dialect: ShellDialect,
    hits: &mut BTreeSet<(u32, &'static str)>,
) {
    let parsed = parse_commands(tokens);
    for cmd in &parsed.commands {
        check_unquoted_vars(tokens, cmd, dialect, code_lines, hits);
        check_eval(tokens, cmd, code_lines, hits);
        check_unchecked_cd(tokens, cmd, scope, code_lines, hits);
        check_unguarded_rm_rf(tokens, cmd, code_lines, hits);
        check_read_without_r(tokens, cmd, code_lines, hits);
        check_insecure_tls(tokens, cmd, code_lines, hits);
        check_shell_process_sub_download(tokens, cmd, code_lines, hits);
    }
    check_download_pipelines(
        tokens,
        &parsed.commands,
        &parsed.pipelines,
        code_lines,
        hits,
    );
}

fn check_unquoted_vars(
    tokens: &[ShellToken],
    cmd: &SimpleCommand,
    dialect: ShellDialect,
    code_lines: &BTreeSet<u32>,
    hits: &mut BTreeSet<(u32, &'static str)>,
) {
    let is_assign_builtin = cmd.cmd_idx.is_some_and(|i| {
        !tokens[i].has_quotes && ASSIGNMENT_BUILTINS.contains(&tokens[i].text.as_str())
    });
    if let Some(cmd_idx) = cmd.cmd_idx {
        emit_unquoted_token(&tokens[cmd_idx], dialect, code_lines, hits);
    }
    for &idx in &cmd.arg_indices {
        let tok = &tokens[idx];
        if is_assign_builtin && is_assignment_word(&tok.text) {
            continue;
        }
        emit_unquoted_token(tok, dialect, code_lines, hits);
    }
    for &idx in &cmd.redir_targets {
        emit_unquoted_token(&tokens[idx], dialect, code_lines, hits);
    }
}

fn emit_unquoted_token(
    tok: &ShellToken,
    dialect: ShellDialect,
    code_lines: &BTreeSet<u32>,
    hits: &mut BTreeSet<(u32, &'static str)>,
) {
    if tok.is_heredoc_delimiter {
        return;
    }
    let lines = if dialect == ShellDialect::Zsh {
        &tok.zsh_split_lines
    } else if tok.has_unquoted_var {
        &tok.unquoted_var_lines
    } else {
        return;
    };
    for &line in lines {
        emit_line(line, "unquoted-variable", code_lines, hits);
    }
}

fn check_eval(
    tokens: &[ShellToken],
    cmd: &SimpleCommand,
    code_lines: &BTreeSet<u32>,
    hits: &mut BTreeSet<(u32, &'static str)>,
) {
    let Some(cmd_idx) = cmd.cmd_idx else {
        return;
    };
    if !tokens[cmd_idx].is_word("eval") {
        return;
    }
    if cmd
        .arg_indices
        .iter()
        .any(|&i| references_variable(&tokens[i]))
    {
        emit_line(tokens[cmd_idx].line, "eval-with-variable", code_lines, hits);
    }
}

fn references_variable(tok: &ShellToken) -> bool {
    tok.has_param_expansion
        || tok.has_backtick
        || tok
            .sub_tokens
            .iter()
            .any(|stream| stream.iter().any(references_variable))
}

fn check_unchecked_cd(
    tokens: &[ShellToken],
    cmd: &SimpleCommand,
    scope: ErrexitScope<'_>,
    code_lines: &BTreeSet<u32>,
    hits: &mut BTreeSet<(u32, &'static str)>,
) {
    let Some(cmd_idx) = cmd.cmd_idx else {
        return;
    };
    if scope.at(cmd_idx) || cmd.in_condition {
        return;
    }
    if !tokens[cmd_idx].is_word("cd") {
        return;
    }
    if matches!(cmd.followed_by_op.as_deref(), Some("&&" | "||")) {
        return;
    }
    emit_line(tokens[cmd_idx].line, "unchecked-cd", code_lines, hits);
}

fn check_download_pipelines(
    tokens: &[ShellToken],
    commands: &[SimpleCommand],
    pipelines: &[Vec<usize>],
    code_lines: &BTreeSet<u32>,
    hits: &mut BTreeSet<(u32, &'static str)>,
) {
    for pipe in pipelines {
        if pipe.len() < 2 {
            continue;
        }
        let mut download_line: Option<u32> = None;
        for &cmd_id in pipe {
            let Some((name, line, _)) = effective_command(tokens, &commands[cmd_id]) else {
                continue;
            };
            if matches!(name, "curl" | "wget") && download_line.is_none() {
                download_line = Some(line);
            } else if let Some(dl_line) = download_line
                && SHELL_INTERPRETERS.contains(&name)
            {
                emit_line(dl_line, "pipe-download-to-shell", code_lines, hits);
                break;
            }
        }
    }
}

fn check_shell_process_sub_download(
    tokens: &[ShellToken],
    cmd: &SimpleCommand,
    code_lines: &BTreeSet<u32>,
    hits: &mut BTreeSet<(u32, &'static str)>,
) {
    let Some((name, line, arg_start)) = effective_command(tokens, cmd) else {
        return;
    };
    if !SHELL_INTERPRETERS.contains(&name) {
        return;
    }
    for &idx in &cmd.arg_indices[arg_start..] {
        let tok = &tokens[idx];
        if tok.is_process_sub
            && tok
                .sub_tokens
                .iter()
                .map(Vec::as_slice)
                .any(stream_starts_downloader)
        {
            emit_line(line, "pipe-download-to-shell", code_lines, hits);
        }
    }
}

fn stream_starts_downloader(sub_tokens: &[ShellToken]) -> bool {
    let parsed = parse_commands(sub_tokens);
    parsed.commands.iter().any(|cmd| {
        effective_command(sub_tokens, cmd)
            .is_some_and(|(name, _, _)| matches!(name, "curl" | "wget"))
    })
}

fn check_unguarded_rm_rf(
    tokens: &[ShellToken],
    cmd: &SimpleCommand,
    code_lines: &BTreeSet<u32>,
    hits: &mut BTreeSet<(u32, &'static str)>,
) {
    let Some((name, line, arg_start)) = effective_command(tokens, cmd) else {
        return;
    };
    if name != "rm" {
        return;
    }
    let mut has_r = false;
    let mut has_f = false;
    let mut paths = Vec::new();
    let mut after_double_dash = false;
    for &idx in &cmd.arg_indices[arg_start..] {
        let tok = &tokens[idx];
        if after_double_dash || !tok.text.starts_with('-') || tok.text == "-" {
            paths.push(tok);
            continue;
        }
        if tok.text == "--" {
            after_double_dash = true;
            continue;
        }
        record_rm_flag(&tok.text, &mut has_r, &mut has_f);
    }
    if has_r && has_f && paths.iter().copied().any(is_unguarded_rm_path) {
        emit_line(line, "unguarded-rm-rf", code_lines, hits);
    }
}

fn record_rm_flag(flag: &str, has_r: &mut bool, has_f: &mut bool) {
    if flag == "--recursive" {
        *has_r = true;
    } else if flag == "--force" {
        *has_f = true;
    } else if let Some(short) = flag.strip_prefix('-')
        && !short.starts_with('-')
    {
        if short.contains('r') || short.contains('R') {
            *has_r = true;
        }
        if short.contains('f') {
            *has_f = true;
        }
    }
}

fn is_unguarded_rm_path(tok: &ShellToken) -> bool {
    if !tok.has_var_expansion {
        return false;
    }
    let trimmed = tok.text.trim_start_matches('"');
    let after_slash = trimmed.strip_prefix('/').unwrap_or(trimmed);
    let Some(rest) = after_slash.strip_prefix('$') else {
        return false;
    };
    if let Some(braced) = rest.strip_prefix('{') {
        return is_unguarded_braced_var_before_slash(braced);
    }
    is_simple_var_before_slash(rest)
}

fn is_unguarded_braced_var_before_slash(braced: &str) -> bool {
    let Some((inner, after_brace)) = braced.split_once('}') else {
        return false;
    };
    if inner.contains(":?") || inner.contains('?') {
        return false;
    }
    let tail = after_brace.trim_start_matches('"');
    tail.starts_with('/')
}

fn is_simple_var_before_slash(rest: &str) -> bool {
    let var_len = rest
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
        .count();
    if var_len == 0 {
        return false;
    }
    let tail = rest[var_len..].trim_start_matches('"');
    tail.starts_with('/')
}

fn check_read_without_r(
    tokens: &[ShellToken],
    cmd: &SimpleCommand,
    code_lines: &BTreeSet<u32>,
    hits: &mut BTreeSet<(u32, &'static str)>,
) {
    let Some(cmd_idx) = cmd.cmd_idx else {
        return;
    };
    if !tokens[cmd_idx].is_word("read") {
        return;
    }
    if !read_args_include_r(tokens, &cmd.arg_indices) {
        emit_line(tokens[cmd_idx].line, "read-without-r", code_lines, hits);
    }
}

fn read_args_include_r(tokens: &[ShellToken], arg_indices: &[usize]) -> bool {
    let mut skip_next = false;
    for &idx in arg_indices {
        if skip_next {
            skip_next = false;
            continue;
        }
        let text = tokens[idx].text.as_str();
        if text == "--" || !text.starts_with('-') || text == "-" {
            break;
        }
        if let Some(short) = text.strip_prefix('-')
            && !short.starts_with('-')
        {
            if short.contains('r') {
                return true;
            }
            if short
                .as_bytes()
                .last()
                .is_some_and(|b| READ_VALUE_SHORT_FLAGS.contains(b))
            {
                skip_next = true;
            }
        }
    }
    false
}

fn check_insecure_tls(
    tokens: &[ShellToken],
    cmd: &SimpleCommand,
    code_lines: &BTreeSet<u32>,
    hits: &mut BTreeSet<(u32, &'static str)>,
) {
    let Some((name, line, arg_start)) = effective_command(tokens, cmd) else {
        return;
    };
    let args = &cmd.arg_indices[arg_start..];
    let insecure = match name {
        "curl" => curl_has_insecure_flag(tokens, args),
        "wget" => args
            .iter()
            .any(|&i| tokens[i].text == "--no-check-certificate"),
        _ => false,
    };
    if insecure {
        emit_line(line, "insecure-tls-download", code_lines, hits);
    }
}

enum CurlFlagStep {
    Insecure,
    Stop,
    SkipNext,
    Continue,
}

fn curl_has_insecure_flag(tokens: &[ShellToken], args: &[usize]) -> bool {
    let mut skip_next = false;
    for &idx in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        match classify_curl_flag(&tokens[idx].text) {
            CurlFlagStep::Insecure => return true,
            CurlFlagStep::Stop => break,
            CurlFlagStep::SkipNext => skip_next = true,
            CurlFlagStep::Continue => {}
        }
    }
    false
}

fn classify_curl_flag(text: &str) -> CurlFlagStep {
    if text == "--" {
        return CurlFlagStep::Stop;
    }
    if text == "--insecure" {
        return CurlFlagStep::Insecure;
    }
    if text.starts_with("--") {
        return if is_curl_long_value_flag(text) {
            CurlFlagStep::SkipNext
        } else {
            CurlFlagStep::Continue
        };
    }
    let Some(short) = text.strip_prefix('-') else {
        return CurlFlagStep::Continue;
    };
    if short.contains('k') {
        CurlFlagStep::Insecure
    } else if short
        .as_bytes()
        .last()
        .is_some_and(|b| CURL_VALUE_SHORT_FLAGS.contains(b))
    {
        CurlFlagStep::SkipNext
    } else {
        CurlFlagStep::Continue
    }
}

fn is_curl_long_value_flag(flag: &str) -> bool {
    !flag.contains('=')
        && matches!(
            flag,
            "--header"
                | "--data"
                | "--data-raw"
                | "--data-binary"
                | "--data-urlencode"
                | "--output"
                | "--request"
                | "--user"
                | "--user-agent"
                | "--cookie"
                | "--cookie-jar"
                | "--form"
                | "--max-time"
                | "--connect-timeout"
                | "--write-out"
                | "--config"
                | "--proxy"
                | "--referer"
                | "--cacert"
                | "--cert"
                | "--key"
        )
}

fn effective_command<'a>(
    tokens: &'a [ShellToken],
    cmd: &SimpleCommand,
) -> Option<(&'a str, u32, usize)> {
    let cmd_idx = cmd.cmd_idx?;
    let cmd_tok = &tokens[cmd_idx];
    let base = path_file_name(&cmd_tok.text);
    if !matches!(base, "sudo" | "env" | "command") {
        return Some((base, cmd_tok.line, 0));
    }
    for (pos, &arg_idx) in cmd.arg_indices.iter().enumerate() {
        let arg = &tokens[arg_idx];
        if arg.text.starts_with('-') || is_assignment_word(&arg.text) {
            continue;
        }
        return Some((path_file_name(&arg.text), arg.line, pos + 1));
    }
    None
}
