# no-hook-bypass

**Invariant**:
Every check that a hook runs also runs in CI on the same files.
Agent sessions refuse commands that skip hooks, such as a no-verify flag or a hooks-path override.

**Implemented by**:
Parity: every hook command in [lefthook.yml](../../lefthook.yml) also runs in the `ci` workflow.
Refusal: the Claude Code `PreToolUse` guard [no-hook-bypass.sh](../../.claude/hooks/no-hook-bypass.sh) runs before every shell command and file write.
It parses shell commands and blocks any prefix of the no-verify flag, a short `-n` flag in a commit's option cluster, `git commit-tree`, a hooks-path override in any letter case, Git configuration from environment variables, every lefthook variable except the verbose and quiet ones, the lefthook uninstall command, a local lefthook override file, and any command that targets `.git/hooks` (including across chained `cd` or `pushd` commands) or a Git hook file such as `hooks/pre-commit`.
Blocked strings inside quoted commit messages or quoted heredocs are allowed, while actual flags, environment variables, and command substitutions are still refused.
[hook_guard.rs](../../xtask/tests/hook_guard.rs) proves each case both ways.
CI also runs on every push to `main`, so a commit that reached `main` without its hooks is still checked.
Codex and OpenCode forward tool calls to the same guard through their project adapters; see [Codex and OpenCode](#codex-and-opencode).

**Runs in**:
CI for parity; the Claude Code, Codex, and OpenCode hook guards for refusal.

**Error text**:

```text
Blocked: this command skips Git hooks. Fix the hook failure and commit again; see docs/checks/no-hook-bypass.md.
Blocked: this file change skips Git hooks. Fix the hook failure and commit again; see docs/checks/no-hook-bypass.md.
Blocked: the hook bypass guard needs jq. Install jq and try again.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/no-hook-bypass.md).

## Codex and OpenCode

Codex CLI runs [no-hook-bypass.sh](../../.codex/hooks/no-hook-bypass.sh) from [hooks.json](../../.codex/hooks.json) on `PreToolUse` for `Bash` and `apply_patch`.
That adapter forwards `Bash` payloads and `apply_patch` target file paths to [no-hook-bypass.sh](../../.claude/hooks/no-hook-bypass.sh).
Codex requires the repository and its project hooks to be trusted before `.codex/hooks.json` executes.

OpenCode loads [no-hook-bypass.js](../../.opencode/plugins/no-hook-bypass.js) as a project plugin under `.opencode/plugins/`.
Its `execute.before` tool hook forwards `bash`, `write`, `edit`, and `apply_patch` calls to [no-hook-bypass.sh](../../.claude/hooks/no-hook-bypass.sh) and throws on any refusal.

[agent_hook_guards.rs](../../xtask/tests/agent_hook_guards.rs) tests both adapters against every case from [hook_guard.rs](../../xtask/tests/hook_guard.rs).
CI parity stays the authoritative gate when a session runs without trusted local hooks.
