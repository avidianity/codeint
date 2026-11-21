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
Codex and OpenCode have no guard yet; CI parity is the first line for them.

**Runs in**:
CI for parity; the Claude Code command guard for refusal.

**Error text**:

```text
Blocked: this command skips Git hooks. Fix the hook failure and commit again; see docs/checks/no-hook-bypass.md.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/no-hook-bypass.md).
