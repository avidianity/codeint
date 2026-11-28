#[cfg(test)]
mod tests;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::iter::Peekable;
use std::str::Chars;

use codeint_model::language::{DeclaredDep, DepKind, PackageDependency};
use serde_json::Value;

const MAX_RUNNER_DEPTH: u8 = 4;

const PACKAGE_BIN_ALIASES: &[(&str, &[&str])] = &[
    ("typescript", &["tsc", "tsserver"]),
    ("@changesets/cli", &["changeset"]),
    ("@playwright/test", &["playwright"]),
    ("@sveltejs/kit", &["svelte-kit"]),
    ("@sveltejs/package", &["svelte-package"]),
    ("@babel/cli", &["babel"]),
    ("@tailwindcss/cli", &["tailwindcss"]),
    ("@commitlint/cli", &["commitlint"]),
    ("@arethetypeswrong/cli", &["attw"]),
    ("@angular/cli", &["ng"]),
    ("@nestjs/cli", &["nest"]),
    ("@docusaurus/core", &["docusaurus"]),
    ("@remix-run/dev", &["remix"]),
    ("@storybook/cli", &["storybook"]),
    ("@astrojs/check", &["astro-check"]),
    ("npm-run-all", &["run-s", "run-p"]),
    ("npm-run-all2", &["npm-run-all", "run-s", "run-p"]),
    ("webpack-cli", &["webpack"]),
    ("postcss-cli", &["postcss"]),
    ("cpy-cli", &["cpy"]),
    ("del-cli", &["del"]),
    ("start-server-and-test", &["start-test"]),
    ("nuxt", &["nuxi"]),
];

const GENERIC_SCOPED_TAILS: &[&str] = &[
    "all",
    "bash",
    "build",
    "bun",
    "cat",
    "check",
    "clean",
    "cli",
    "config",
    "core",
    "coverage",
    "cp",
    "dev",
    "dist",
    "docs",
    "echo",
    "env",
    "fix",
    "format",
    "git",
    "lint",
    "loader",
    "mkdir",
    "mv",
    "node",
    "npm",
    "npx",
    "package",
    "plugin",
    "pnpm",
    "preset",
    "publish",
    "release",
    "rm",
    "run",
    "serve",
    "sh",
    "src",
    "start",
    "test",
    "tests",
    "typecheck",
    "types",
    "utils",
    "watch",
    "yarn",
];

const DIST_TAGS: &[&str] = &["latest", "next", "beta", "alpha", "canary", "rc"];

pub(crate) fn extract_manifest_used_dependencies(
    parsed_manifests: &[(&str, &str, Value)],
    workspace_members: &[String],
    declared: &[DeclaredDep],
) -> Vec<PackageDependency> {
    let tokens_by_root = collect_tokens_by_root(parsed_manifests, workspace_members);
    if tokens_by_root.is_empty() {
        return Vec::new();
    }
    let mut used = BTreeSet::new();
    for dep in declared {
        if dep.kind != DepKind::Dev {
            continue;
        }
        if let Some(tokens) = tokens_by_root.get(dep.package_root.as_str())
            && is_invoked_in_scripts(&dep.name, tokens)
        {
            used.insert((dep.package_root.clone(), dep.name.clone()));
        }
    }
    used.into_iter()
        .map(|(package_root, name)| PackageDependency { package_root, name })
        .collect()
}

fn collect_tokens_by_root<'a>(
    parsed_manifests: &[(&'a str, &'a str, Value)],
    workspace_members: &[String],
) -> HashMap<&'a str, HashSet<String>> {
    let mut per_package: HashMap<&'a str, HashSet<String>> = HashMap::new();
    for (root, _, doc) in parsed_manifests {
        let Some(scripts) = doc.get("scripts").and_then(Value::as_object) else {
            continue;
        };
        let entry = per_package.entry(*root).or_default();
        for command in scripts.values().filter_map(Value::as_str) {
            extract_commands_from_script(command, entry, 0);
        }
    }
    if per_package.contains_key("") {
        let member_roots: HashSet<&str> = workspace_members.iter().map(String::as_str).collect();
        let member_tokens: Vec<String> = per_package
            .iter()
            .filter(|(root, _)| !root.is_empty() && member_roots.contains(*root))
            .flat_map(|(_, tokens)| tokens.iter().cloned())
            .collect();
        if let Some(root_tokens) = per_package.get_mut("") {
            root_tokens.extend(member_tokens);
        }
    }
    per_package
}

fn extract_commands_from_script(script: &str, out: &mut HashSet<String>, depth: u8) {
    if depth >= MAX_RUNNER_DEPTH {
        return;
    }
    for segment in ShellLexer::new(script).tokenize() {
        extract_commands_from_segment(&segment, out, depth);
    }
}

fn extract_commands_from_segment(words: &[ShellWord], out: &mut HashSet<String>, depth: u8) {
    if depth >= MAX_RUNNER_DEPTH {
        return;
    }
    let start = skip_leading_assignments(words);
    let Some(first) = words.get(start) else {
        return;
    };
    let rest = &words[start + 1..];
    if first.text == "!" && !first.starts_quoted {
        extract_commands_from_segment(rest, out, depth + 1);
        return;
    }
    let cmd = normalize_command_token(&first.text);
    if cmd.is_empty() || cmd.starts_with('-') {
        return;
    }
    out.insert(cmd.to_owned());
    unwrap_runner(cmd, rest, out, depth);
}

fn skip_leading_assignments(words: &[ShellWord]) -> usize {
    words
        .iter()
        .take_while(|word| !word.starts_quoted && is_env_assignment(&word.text))
        .count()
}

fn unwrap_runner(cmd: &str, rest: &[ShellWord], out: &mut HashSet<String>, depth: u8) {
    match cmd {
        "env" | "cross-env" | "time" | "nice" | "nohup" => {
            unwrap_prefix_runner(cmd, rest, out, depth);
        }
        "npx" | "pnpx" | "bunx" => unwrap_exec_runner(rest, out, depth),
        "pnpm" | "yarn" | "npm" | "bun" => unwrap_package_manager(cmd, rest, out, depth),
        "sh" | "bash" | "zsh" | "dash" => unwrap_shell_runner(rest, out, depth),
        "concurrently" => unwrap_concurrently(rest, out, depth),
        _ => {}
    }
}

fn unwrap_prefix_runner(cmd: &str, rest: &[ShellWord], out: &mut HashSet<String>, depth: u8) {
    let mut index = 0;
    while let Some(word) = rest.get(index) {
        let flag = word.text.as_str();
        if flag == "--" {
            index += 1;
            break;
        }
        if cmd == "env" && matches!(flag, "-S" | "--split-string") {
            extract_next_script(rest, index + 1, out, depth);
            return;
        }
        if !flag.starts_with('-') {
            break;
        }
        index += flag_step(prefix_flag_takes_value(cmd, flag));
    }
    recurse_segment_from(rest, index, out, depth);
}

fn unwrap_exec_runner(rest: &[ShellWord], out: &mut HashSet<String>, depth: u8) {
    let mut index = 0;
    while let Some(word) = rest.get(index) {
        let flag = word.text.as_str();
        if flag == "--" {
            index += 1;
            break;
        }
        if let Some(script) = flag.strip_prefix("--call=") {
            extract_commands_from_script(script, out, depth + 1);
            return;
        }
        if matches!(flag, "-c" | "--call") {
            extract_next_script(rest, index + 1, out, depth);
            return;
        }
        if !flag.starts_with('-') {
            break;
        }
        index += flag_step(exec_flag_takes_value(flag));
    }
    recurse_segment_from(rest, index, out, depth);
}

fn unwrap_package_manager(cmd: &str, rest: &[ShellWord], out: &mut HashSet<String>, depth: u8) {
    let mut index = 0;
    while let Some(word) = rest.get(index) {
        let flag = word.text.as_str();
        if !flag.starts_with('-') {
            break;
        }
        index += flag_step(pm_flag_takes_value(flag));
    }
    let Some(subcommand) = rest.get(index) else {
        return;
    };
    let is_exec = matches!(subcommand.text.as_str(), "exec" | "dlx")
        || (cmd == "bun" && subcommand.text == "x");
    if is_exec {
        unwrap_exec_runner(&rest[index + 1..], out, depth);
    }
}

fn unwrap_shell_runner(rest: &[ShellWord], out: &mut HashSet<String>, depth: u8) {
    let mut index = 0;
    let mut saw_command_flag = false;
    while let Some(word) = rest.get(index) {
        let flag = word.text.as_str();
        if flag == "--" {
            index += 1;
            break;
        }
        if is_shell_command_flag(flag) {
            saw_command_flag = true;
            index += 1;
        } else if matches!(flag, "-o" | "+o") {
            index += 2;
        } else if flag.starts_with('-') || flag.starts_with('+') {
            index += 1;
        } else {
            break;
        }
    }
    if saw_command_flag {
        extract_next_script(rest, index, out, depth);
    }
}

fn unwrap_concurrently(rest: &[ShellWord], out: &mut HashSet<String>, depth: u8) {
    let mut index = 0;
    let mut after_double_dash = false;
    while let Some(word) = rest.get(index) {
        let text = word.text.as_str();
        if !after_double_dash && text == "--" {
            after_double_dash = true;
            index += 1;
        } else if !after_double_dash && text.starts_with('-') {
            index += flag_step(concurrently_flag_takes_value(text));
        } else {
            extract_commands_from_script(text, out, depth + 1);
            index += 1;
        }
    }
}

fn extract_next_script(rest: &[ShellWord], index: usize, out: &mut HashSet<String>, depth: u8) {
    if let Some(arg) = rest.get(index) {
        extract_commands_from_script(&arg.text, out, depth + 1);
    }
}

fn recurse_segment_from(rest: &[ShellWord], index: usize, out: &mut HashSet<String>, depth: u8) {
    if index < rest.len() {
        extract_commands_from_segment(&rest[index..], out, depth + 1);
    }
}

fn flag_step(takes_value: bool) -> usize {
    if takes_value { 2 } else { 1 }
}

fn prefix_flag_takes_value(cmd: &str, flag: &str) -> bool {
    match cmd {
        "env" => matches!(flag, "-u" | "--unset" | "-C" | "--chdir"),
        "nice" => matches!(flag, "-n" | "--adjustment"),
        "time" => matches!(flag, "-f" | "--format" | "-o" | "--output"),
        _ => false,
    }
}

fn exec_flag_takes_value(flag: &str) -> bool {
    matches!(
        flag,
        "-p" | "--package" | "--prefix" | "--userconfig" | "--cache" | "--filter" | "-F"
    )
}

fn pm_flag_takes_value(flag: &str) -> bool {
    matches!(
        flag,
        "--filter" | "-F" | "--dir" | "-C" | "--prefix" | "--cwd" | "--workspace" | "--config"
    )
}

fn concurrently_flag_takes_value(flag: &str) -> bool {
    matches!(
        flag,
        "-n" | "--names"
            | "--name-separator"
            | "-c"
            | "--prefix-colors"
            | "-p"
            | "--prefix"
            | "-l"
            | "--prefix-length"
            | "-t"
            | "--timestamp-format"
            | "-m"
            | "--max-processes"
            | "-s"
            | "--success"
            | "--hide"
            | "--restart-tries"
            | "--restart-after"
            | "--default-input-target"
            | "--cwd"
    )
}

fn is_shell_command_flag(flag: &str) -> bool {
    flag == "-c"
        || (flag.starts_with('-')
            && !flag.starts_with("--")
            && flag[1..].bytes().all(|byte| byte.is_ascii_alphabetic())
            && flag[1..].contains('c'))
}

#[derive(Debug, PartialEq, Eq)]
struct ShellWord {
    text: String,
    starts_quoted: bool,
}

struct ShellLexer<'a> {
    chars: Peekable<Chars<'a>>,
    segments: Vec<Vec<ShellWord>>,
    current_segment: Vec<ShellWord>,
    current_word: String,
    in_word: bool,
    starts_quoted: bool,
    skip_next_word: bool,
}

impl<'a> ShellLexer<'a> {
    fn new(input: &'a str) -> ShellLexer<'a> {
        ShellLexer {
            chars: input.chars().peekable(),
            segments: Vec::new(),
            current_segment: Vec::new(),
            current_word: String::new(),
            in_word: false,
            starts_quoted: false,
            skip_next_word: false,
        }
    }

    fn tokenize(mut self) -> Vec<Vec<ShellWord>> {
        while let Some(ch) = self.chars.next() {
            self.handle_char(ch);
        }
        self.flush_segment();
        self.segments
    }

    fn handle_char(&mut self, ch: char) {
        if matches!(ch, ' ' | '\t') {
            self.flush_word();
        } else if matches!(ch, '\n' | '\r' | ';' | '(' | ')' | '&' | '|') {
            self.consume_separator(ch);
        } else if matches!(ch, '>' | '<') {
            self.consume_redirection(ch);
        } else {
            self.handle_word_char(ch);
        }
    }

    fn handle_word_char(&mut self, ch: char) {
        match ch {
            '\'' => self.consume_single_quote(),
            '"' => self.consume_double_quote(),
            '\\' => self.consume_backslash(),
            '#' if !self.in_word => self.consume_comment(),
            _ => self.push_char(ch),
        }
    }

    fn start_word(&mut self, quoted: bool) {
        if !self.in_word {
            self.in_word = true;
            self.starts_quoted = quoted;
        }
    }

    fn push_char(&mut self, ch: char) {
        self.start_word(false);
        self.current_word.push(ch);
    }

    fn consume_single_quote(&mut self) {
        self.start_word(true);
        for ch in self.chars.by_ref() {
            if ch == '\'' {
                break;
            }
            self.current_word.push(ch);
        }
    }

    fn consume_double_quote(&mut self) {
        self.start_word(true);
        while let Some(ch) = self.chars.next() {
            if ch == '"' {
                break;
            }
            if ch == '\\' {
                self.consume_double_quote_escape();
            } else {
                self.current_word.push(ch);
            }
        }
    }

    fn consume_double_quote_escape(&mut self) {
        let Some(next) = self.chars.next() else {
            self.current_word.push('\\');
            return;
        };
        if matches!(next, '"' | '\\' | '$' | '`') {
            self.current_word.push(next);
        } else if next != '\n' {
            self.current_word.push('\\');
            self.current_word.push(next);
        }
    }

    fn consume_backslash(&mut self) {
        let Some(next) = self.chars.next() else {
            self.push_char('\\');
            return;
        };
        if next != '\n' {
            self.start_word(true);
            self.current_word.push(next);
        }
    }

    fn consume_comment(&mut self) {
        for ch in self.chars.by_ref() {
            if ch == '\n' {
                self.flush_segment();
                break;
            }
        }
    }

    fn consume_separator(&mut self, ch: char) {
        if matches!(ch, '&' | '|') && self.chars.peek() == Some(&ch) {
            self.chars.next();
        }
        self.flush_segment();
    }

    fn consume_redirection(&mut self, ch: char) {
        let is_fd_prefix = self.in_word
            && !self.starts_quoted
            && !self.current_word.is_empty()
            && self.current_word.bytes().all(|byte| byte.is_ascii_digit());
        if is_fd_prefix {
            self.reset_word();
        } else {
            self.flush_word();
        }
        self.consume_redirection_tail(ch);
        self.skip_next_word = true;
    }

    fn consume_redirection_tail(&mut self, ch: char) {
        let Some(&next) = self.chars.peek() else {
            return;
        };
        if next == ch || next == '&' || (ch == '<' && next == '>') {
            self.chars.next();
            if ch == '<' && next == '<' && self.chars.peek() == Some(&'<') {
                self.chars.next();
            }
        }
    }

    fn reset_word(&mut self) {
        self.current_word.clear();
        self.in_word = false;
        self.starts_quoted = false;
    }

    fn flush_word(&mut self) {
        if !self.in_word {
            return;
        }
        let word = ShellWord {
            text: std::mem::take(&mut self.current_word),
            starts_quoted: self.starts_quoted,
        };
        self.in_word = false;
        self.starts_quoted = false;
        if self.skip_next_word {
            self.skip_next_word = false;
            return;
        }
        self.current_segment.push(word);
    }

    fn flush_segment(&mut self) {
        self.flush_word();
        self.skip_next_word = false;
        if !self.current_segment.is_empty() {
            self.segments
                .push(std::mem::take(&mut self.current_segment));
        }
    }
}

fn is_env_assignment(token: &str) -> bool {
    let Some((name, _)) = token.split_once('=') else {
        return false;
    };
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn normalize_command_token(token: &str) -> &str {
    let bin = strip_node_modules_bin_prefix(token);
    strip_valid_package_version(bin)
}

fn strip_node_modules_bin_prefix(token: &str) -> &str {
    if let Some(rest) = token.strip_prefix("node_modules/.bin/") {
        return rest;
    }
    if let Some((_, rest)) = token.rsplit_once("/node_modules/.bin/") {
        return rest;
    }
    token
}

fn strip_valid_package_version(bin: &str) -> &str {
    if let Some(rest) = bin.strip_prefix('@') {
        let Some((scope, after_scope)) = rest.split_once('/') else {
            return bin;
        };
        let Some((name, version)) = after_scope.split_once('@') else {
            return bin;
        };
        if is_valid_pkg_segment(scope)
            && is_valid_pkg_segment(name)
            && is_valid_version_or_tag(version)
        {
            return &bin[..bin.len() - version.len() - 1];
        }
        return bin;
    }
    let Some((name, version)) = bin.split_once('@') else {
        return bin;
    };
    if is_valid_pkg_segment(name) && is_valid_version_or_tag(version) {
        return name;
    }
    bin
}

fn is_valid_pkg_segment(segment: &str) -> bool {
    !segment.is_empty()
        && !segment.starts_with('.')
        && !segment.starts_with('-')
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn is_valid_version_or_tag(version: &str) -> bool {
    let Some(first) = version.bytes().next() else {
        return false;
    };
    if version.contains('/') || version.contains('@') {
        return false;
    }
    if DIST_TAGS.contains(&version) {
        return true;
    }
    first.is_ascii_digit() || matches!(first, b'^' | b'~' | b'*' | b'=' | b'<' | b'>')
}

fn is_invoked_in_scripts(package_name: &str, script_tokens: &HashSet<String>) -> bool {
    if script_tokens.contains(package_name) {
        return true;
    }
    if let Some((_, aliases)) = PACKAGE_BIN_ALIASES
        .iter()
        .find(|&&(pkg, _)| pkg == package_name)
        && aliases.iter().any(|alias| script_tokens.contains(*alias))
    {
        return true;
    }
    if let Some((_, tail)) = package_name
        .strip_prefix('@')
        .and_then(|rest| rest.split_once('/'))
        && tail.len() >= 3
        && !GENERIC_SCOPED_TAILS.contains(&tail)
        && script_tokens.contains(tail)
    {
        return true;
    }
    false
}
