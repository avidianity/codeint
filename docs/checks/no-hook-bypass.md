# no-hook-bypass

**Invariant**:
Every check that a hook runs also runs in CI on the same files.
Agent sessions refuse commands that skip hooks, such as a no-verify flag or a hooks-path override.

**Implemented by**:
Parity: every hook command in [lefthook.yml](../../lefthook.yml) also runs in the `ci` workflow.
Refusal: the Claude Code `PreToolUse` guard [no-hook-bypass.sh](../../.claude/hooks/no-hook-bypass.sh) blocks a Git command with the no-verify flag, the short `-n` flag of a commit, a hooks-path override, and the lefthook skip variables.
Codex and OpenCode have no guard yet; CI parity is the first line for them.

**Runs in**:
CI for parity; the Claude Code command guard for refusal.

**Error text**:

```text
{check} failed in CI. A local hook should have caught this. Never skip hooks; fix the failure and commit again.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/no-hook-bypass.md).
