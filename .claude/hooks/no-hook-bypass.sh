#!/usr/bin/env bash
set -euo pipefail

input="$(cat)"
if ! command -v jq >/dev/null 2>&1; then
  echo "Blocked: the hook bypass guard needs jq. Install jq and try again." >&2
  exit 2
fi
if ! command -v python3 >/dev/null 2>&1; then
  echo "Blocked: the hook bypass guard needs python3. Install python3 and try again." >&2
  exit 2
fi
command_text="$(printf '%s' "$input" | jq -r '.tool_input.command // ""')"
file_path="$(printf '%s' "$input" | jq -r '.tool_input.file_path // .tool_input.notebook_path // ""')"

refuse() {
  echo "Blocked: this $1 skips Git hooks. Fix the hook failure and commit again; see docs/checks/no-hook-bypass.md." >&2
  exit 2
}

case "$file_path" in
  *lefthook-local* | */.git/hooks/* | .git/hooks/*) refuse "file change" ;;
esac

[ -n "$command_text" ] || exit 0

if ! python3 - "$command_text" <<'PY'
import os
import re
import sys

HASH = chr(35)
HOOK_FILE_RE = re.compile(
    r"(^|/)hooks/(pre-commit|commit-msg|prepare-commit-msg|pre-push|pre-merge-commit|post-commit|pre-rebase)($|/)",
    re.IGNORECASE,
)
NO_VERIFY_RE = re.compile(r"^--no-v[a-z]*$", re.IGNORECASE)
ENV_ASSIGN_RE = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*)=(.*)$", re.DOTALL)
REDIR_OPS = (">>|", "&>>", "<<-", "<<<", ">>", "<<", "<>", ">|", ">&", "<&", "&>", ">", "<")
SEP_OPS = ("&&", "||", "|&", ";", "|", "&", "\n")
GIT_GLOBAL_VALUE_FLAGS = {
    "-C",
    "-c",
    "--git-dir",
    "--work-tree",
    "--namespace",
    "--super-prefix",
    "--config-env",
}
GIT_MESSAGE_SUBCOMMANDS = {"commit", "tag", "merge", "notes", "revert", "cherry-pick"}
SHELL_WRAPPERS = {"command", "builtin", "exec", "time", "nohup", "sudo"}
SUBSHELL_BINS = {"sh", "bash", "zsh", "dash", "ksh"}
JS_RUNNERS = {"npx", "pnpm", "yarn", "bunx"}


class Refused(Exception):
    pass


def resolve_path(cwd, raw):
    if raw.startswith("/"):
        parts = []
    else:
        parts = list(cwd)
    for seg in raw.split("/"):
        if not seg or seg == ".":
            continue
        if seg == "..":
            if parts:
                parts.pop()
        else:
            parts.append(seg)
    return parts


def is_git_hooks_path(parts):
    lower = [p.lower() for p in parts]
    for idx in range(len(lower) - 1):
        if lower[idx] == ".git" and lower[idx + 1] == "hooks":
            return True
    return False


def check_env_assignment(name):
    upper = name.upper()
    if upper.startswith("GIT_CONFIG"):
        raise Refused()
    if upper.startswith("LEFTHOOK") and upper not in ("LEFTHOOK_VERBOSE", "LEFTHOOK_QUIET"):
        raise Refused()


def extract_heredoc_substitutions(body):
    subs = []
    i = 0
    n = len(body)
    while i < n:
        ch = body[i]
        if ch == "\\" and i + 1 < n:
            i += 2
            continue
        if ch == "$" and i + 1 < n and body[i + 1] == "(":
            sub, i = read_dollar_paren(body, i + 2)
            subs.append(sub)
            continue
        if ch == "`":
            sub, i = read_backtick(body, i + 1, in_dquote=True)
            subs.append(sub)
            continue
        i += 1
    return subs


def read_backtick(src, i, in_dquote=False):
    buf = []
    n = len(src)
    while i < n:
        ch = src[i]
        if ch == "\\" and i + 1 < n:
            nxt = src[i + 1]
            if nxt in ("$", "`", "\\") or (in_dquote and nxt == '"'):
                buf.append(nxt)
                i += 2
                continue
        if ch == "`":
            return "".join(buf), i + 1
        buf.append(ch)
        i += 1
    return "".join(buf), i


def read_dollar_paren(src, i):
    _, end = tokenize(src, i, stop_at_rparen=True)
    if end > i and src[end - 1] == ")":
        return src[i : end - 1], end
    return src[i:end], end


def consume_heredocs(src, i, pending):
    subs = []
    n = len(src)
    for delim, quoted, strip_tabs in pending:
        lines = []
        while i < n:
            nl = src.find("\n", i)
            if nl == -1:
                line = src[i:]
                i = n
            else:
                line = src[i:nl]
                i = nl + 1
            candidate = line.lstrip("\t") if strip_tabs else line
            if candidate == delim:
                break
            lines.append(line)
        if not quoted:
            subs.extend(extract_heredoc_substitutions("\n".join(lines)))
    return i, subs


def tokenize(src, start=0, stop_at_rparen=False):
    tokens = []
    pending_heredocs = []
    i = start
    n = len(src)
    paren_depth = 0

    while i < n:
        ch = src[i]
        if ch in (" ", "\t", "\r"):
            i += 1
            continue
        if ch == "\n":
            tokens.append(("op", "\n", [], False))
            i += 1
            if pending_heredocs:
                i, hsubs = consume_heredocs(src, i, pending_heredocs)
                pending_heredocs = []
                if hsubs:
                    tokens.append(("heredoc_subs", "", hsubs, False))
            continue
        if ch == HASH:
            nl = src.find("\n", i)
            i = n if nl == -1 else nl
            continue
        if ch == "(":
            paren_depth += 1
            tokens.append(("op", "(", [], False))
            i += 1
            continue
        if ch == ")":
            if stop_at_rparen and paren_depth == 0:
                return tokens, i + 1
            if paren_depth > 0:
                paren_depth -= 1
            tokens.append(("op", ")", [], False))
            i += 1
            continue

        matched_sep = None
        for op in SEP_OPS:
            if op != "\n" and src.startswith(op, i):
                matched_sep = op
                break
        if matched_sep is not None:
            tokens.append(("op", matched_sep, [], False))
            i += len(matched_sep)
            continue

        j = i
        while j < n and src[j].isdigit():
            j += 1
        matched_redir = None
        for rop in REDIR_OPS:
            if src.startswith(rop, j):
                matched_redir = rop
                break
        if matched_redir is not None and (j == i or matched_redir in (">", ">>", "<", "<>", ">|", ">&", "<&", "<<", "<<-", "<<<")):
            tokens.append(("redir", matched_redir, [], False))
            i = j + len(matched_redir)
            continue

        buf = []
        subs = []
        was_quoted = False
        while i < n:
            c = src[i]
            if c in (" ", "\t", "\r", "\n", "(", ")", ";", "|", "&", ">", "<"):
                break
            if c == "\\" and i + 1 < n:
                if src[i + 1] == "\n":
                    i += 2
                    continue
                buf.append(src[i + 1])
                was_quoted = True
                i += 2
                continue
            if c == "'":
                was_quoted = True
                i += 1
                while i < n and src[i] != "'":
                    buf.append(src[i])
                    i += 1
                if i < n:
                    i += 1
                continue
            if c == "$" and i + 1 < n and src[i + 1] == "'":
                was_quoted = True
                i += 2
                while i < n and src[i] != "'":
                    if src[i] == "\\" and i + 1 < n:
                        buf.append(src[i + 1])
                        i += 2
                    else:
                        buf.append(src[i])
                        i += 1
                if i < n:
                    i += 1
                continue
            if c == '"':
                was_quoted = True
                i += 1
                while i < n and src[i] != '"':
                    dc = src[i]
                    if dc == "\\" and i + 1 < n:
                        nxt = src[i + 1]
                        if nxt in ('"', "\\", "$", "`"):
                            buf.append(nxt)
                            i += 2
                            continue
                        if nxt == "\n":
                            i += 2
                            continue
                    if dc == "$" and i + 1 < n and src[i + 1] == "(":
                        sub, i = read_dollar_paren(src, i + 2)
                        subs.append(sub)
                        continue
                    if dc == "`":
                        sub, i = read_backtick(src, i + 1, in_dquote=True)
                        subs.append(sub)
                        continue
                    buf.append(dc)
                    i += 1
                if i < n:
                    i += 1
                continue
            if c == "$" and i + 1 < n and src[i + 1] == "(":
                sub, i = read_dollar_paren(src, i + 2)
                subs.append(sub)
                continue
            if c == "`":
                sub, i = read_backtick(src, i + 1, in_dquote=False)
                subs.append(sub)
                continue
            buf.append(c)
            i += 1

        word = "".join(buf)
        if tokens and tokens[-1][0] == "redir" and tokens[-1][1] in ("<<", "<<-"):
            strip_tabs = tokens[-1][1] == "<<-"
            pending_heredocs.append((word, was_quoted, strip_tabs))
            tokens.append(("heredoc_delim", word, subs, was_quoted))
        else:
            tokens.append(("word", word, subs, was_quoted))

    return tokens, i


def check_path_token(cwd, token):
    if "lefthook-local" in token.lower():
        raise Refused()
    if HOOK_FILE_RE.search(token):
        raise Refused()
    if is_git_hooks_path(resolve_path(cwd, token)):
        raise Refused()
    if "=" in token:
        _, val = token.split("=", 1)
        if val:
            if "lefthook-local" in val.lower():
                raise Refused()
            if HOOK_FILE_RE.search(val):
                raise Refused()
            if is_git_hooks_path(resolve_path(cwd, val)):
                raise Refused()


def check_git_command(args):
    message_indices = set()
    i = 0
    n = len(args)
    while i < n:
        arg = args[i]
        if arg == "--":
            i += 1
            break
        if arg in GIT_GLOBAL_VALUE_FLAGS:
            if arg in ("-c", "--config-env") and i + 1 < n:
                if "core.hookspath" in args[i + 1].lower():
                    raise Refused()
            i += 2
            continue
        if arg.startswith("-c") and len(arg) > 2:
            if "core.hookspath" in arg[2:].lower():
                raise Refused()
            i += 1
            continue
        if arg.startswith("--config-env="):
            if "core.hookspath" in arg.lower():
                raise Refused()
            i += 1
            continue
        if arg.startswith("-"):
            i += 1
            continue
        break

    if i >= n:
        return message_indices

    subcmd = args[i].lower()
    if subcmd == "commit-tree":
        raise Refused()

    after_dd = False
    expect_msg = False
    for idx in range(i + 1, n):
        arg = args[idx]
        if expect_msg:
            message_indices.add(idx)
            expect_msg = False
            continue
        if after_dd:
            continue
        if arg == "--":
            after_dd = True
            continue
        if NO_VERIFY_RE.match(arg):
            raise Refused()
        if "core.hookspath" in arg.lower():
            raise Refused()
        if subcmd in GIT_MESSAGE_SUBCOMMANDS:
            if arg in ("-m", "--message"):
                expect_msg = True
                continue
            if arg.startswith("--message="):
                message_indices.add(idx)
                continue
        if subcmd == "commit" and arg.startswith("-") and not arg.startswith("--") and len(arg) > 1:
            cluster = arg[1:]
            for cidx, ch in enumerate(cluster):
                if ch == "m":
                    if cidx == len(cluster) - 1:
                        expect_msg = True
                    else:
                        message_indices.add(idx)
                    break
                if ch.lower() == "n":
                    raise Refused()
    return message_indices


def check_simple_command(words, redir_targets, cwd, dir_stack):
    for target in redir_targets:
        check_path_token(cwd, target)

    idx = 0
    n = len(words)
    while idx < n:
        match = ENV_ASSIGN_RE.match(words[idx])
        if match:
            check_env_assignment(match.group(1))
            idx += 1
        else:
            break

    while idx < n:
        cmd_base = os.path.basename(words[idx]).lower()
        if cmd_base == "env":
            idx += 1
            while idx < n:
                w = words[idx]
                if w == "--":
                    idx += 1
                    break
                if w in ("-u", "-C", "-S"):
                    idx += 2
                    continue
                if w.startswith("-"):
                    idx += 1
                    continue
                match = ENV_ASSIGN_RE.match(w)
                if match:
                    check_env_assignment(match.group(1))
                    idx += 1
                    continue
                break
            continue
        if cmd_base in SHELL_WRAPPERS:
            idx += 1
            while idx < n and words[idx].startswith("-"):
                if words[idx] == "--":
                    idx += 1
                    break
                idx += 1
            continue
        break

    if idx >= n:
        return cwd

    cmd_word = words[idx]
    cmd_base = os.path.basename(cmd_word).lower()
    args = words[idx + 1 :]

    if cmd_base in ("export", "declare", "typeset"):
        for a in args:
            match = ENV_ASSIGN_RE.match(a)
            if match:
                check_env_assignment(match.group(1))

    if cmd_base in ("cd", "pushd"):
        target = None
        skip_flags = True
        for a in args:
            if skip_flags and a == "--":
                skip_flags = False
                continue
            if skip_flags and a.startswith("-") and a != "-":
                continue
            target = a
            break
        if target is None or target == "-":
            new_cwd = ["__home__"]
        else:
            new_cwd = resolve_path(cwd, target)
        if cmd_base == "pushd":
            dir_stack.append(list(cwd))
        cwd = new_cwd
        if is_git_hooks_path(cwd):
            raise Refused()
        return cwd

    if cmd_base == "popd":
        if dir_stack:
            cwd = dir_stack.pop()
            if is_git_hooks_path(cwd):
                raise Refused()
        return cwd

    if cmd_base in SUBSHELL_BINS:
        for aidx, a in enumerate(args):
            if a == "--":
                break
            if a.startswith("-") and not a.startswith("--") and "c" in a[1:]:
                if aidx + 1 < len(args):
                    analyze(args[aidx + 1], list(cwd), list(dir_stack))
                break

    message_indices = set()
    if cmd_base == "git":
        message_indices = check_git_command(args)
    elif cmd_base == "lefthook":
        if any(a.lower() == "uninstall" for a in args):
            raise Refused()
    elif cmd_base in JS_RUNNERS:
        lower_args = [a.lower() for a in args]
        if "lefthook" in lower_args:
            lh_idx = lower_args.index("lefthook")
            if "uninstall" in lower_args[lh_idx + 1 :]:
                raise Refused()

    check_path_token(cwd, cmd_word)
    for aidx, a in enumerate(args):
        if aidx in message_indices:
            continue
        if NO_VERIFY_RE.match(a):
            raise Refused()
        if "core.hookspath" in a.lower():
            raise Refused()
        check_path_token(cwd, a)

    return cwd


def analyze(src, cwd, dir_stack):
    tokens, _ = tokenize(src)
    saved_states = []
    words = []
    redir_targets = []
    expect_redir = None

    def flush():
        nonlocal cwd, words, redir_targets
        if words or redir_targets:
            cwd = check_simple_command(words, redir_targets, cwd, dir_stack)
            words = []
            redir_targets = []

    for kind, text, subs, _ in tokens:
        for sub in subs:
            analyze(sub, list(cwd), list(dir_stack))
        if kind == "heredoc_subs":
            continue
        if kind == "heredoc_delim":
            expect_redir = None
            continue
        if kind == "op":
            flush()
            expect_redir = None
            if text == "(":
                saved_states.append((list(cwd), [list(d) for d in dir_stack]))
            elif text == ")" and saved_states:
                cwd, dir_stack = saved_states.pop()
            continue
        if kind == "redir":
            expect_redir = text
            continue
        if kind == "word":
            if text in ("{", "}") and not words and expect_redir is None:
                continue
            if expect_redir is not None:
                if expect_redir not in ("<<<", "<<", "<<-", ">&", "<&"):
                    redir_targets.append(text)
                expect_redir = None
            else:
                words.append(text)

    flush()


try:
    analyze(sys.argv[1], ["__root__"], [])
except Refused:
    sys.exit(2)
PY
then
  refuse "command"
fi
exit 0
