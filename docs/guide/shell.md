# Shell

codeint ships a built-in `shell` Language plugin that measures every `.sh`, `.bash`, and `.zsh` file with no configuration required.
It measures Erosion, Cognitive complexity, Duplication, Verbosity, and Hazards.
Shell has no imports, so Import cycles, Import boundaries, Orphan files, and Dependency hygiene are absent for shell scripts rather than `0`.
The `shell` plugin uses a hand-written lexer and adds no external parser dependency.

## Shell flavors

The `shell` plugin detects the flavor of each file from its first-line shebang, then from its extension:

- **Shebang**: `#!/bin/sh`, `#!/bin/bash`, `#!/bin/zsh`, `#!/usr/bin/env bash`, `#!/bin/busybox sh`, and equivalent interpreter paths select POSIX `sh`, Bash, or Zsh.
  `ash` and `dash` count as POSIX `sh`.
- **Extension**: without a recognized shebang, `.bash` selects Bash, `.zsh` selects Zsh, and `.sh` selects POSIX `sh`.

The flavor decides which strict-mode flags `missing-strict-mode` expects, and whether `posix-sh-bashism` applies.

## What counts as a function

- **Named functions**: each `name() { ... }` and `function name { ... }` definition is one function with its own name.
- **Top-level script body**: when a file has executable commands outside named functions, those commands form one extra function named `<script>`.
  A 300-line flat script with no function definitions still shows up in Erosion and Cognitive complexity.
- **Decision points**: `if`, `elif`, `for`, `while`, `until`, `select`, each `case` branch except a lone `*)` wildcard, `&&`, and `||` each add `1`.
  Nested control flow raises Cognitive complexity by its nesting depth.
- **Source lines**: comments and blank lines are not source lines.
  Heredoc bodies, quoted strings, `$(...)` substitutions, ANSI-C `$'...'` strings, and line continuations are read without false syntax errors.

## Source sets

After checking any `source_sets` globs in `codeint.jsonc` or `codeint.json` (which always override built-in rules), the `shell` plugin classifies each file as follows:

- **Vendored**: any file under a `vendor/` or `third_party/` folder.
- **Generated**: Autotools helper scripts such as `ltmain.sh` and `configure.sh`, and any file whose header comment contains a generated-file marker such as `Code generated ... DO NOT EDIT.`, `do not modify by hand`, or `this file is generated`.
- **Test**: any file under a `test/`, `tests/`, or `spec/` folder, or any file named `test_*.sh`, `*_test.sh`, or `*.test.sh` (and the same names with `.bash` or `.zsh`).
- **Production**: every other `.sh`, `.bash`, or `.zsh` file.

codeint dispatches files by extension, so extensionless scripts such as `configure` or `bin/deploy` are not measured.

## Workspace packages

Shell files have no manifest of their own.
Each shell file belongs to the enclosing Workspace package that another Language plugin declares, such as a `package.json` workspace, a `Cargo.toml` member, or a `go.mod` folder.
Select one Workspace package with `--package <path>`.

## Flagged line rules (Verbosity)

| Rule | Matches |
| ---- | ------- |
| `useless-cat` | `cat file \| command` with exactly one plain file operand |
| `legacy-backticks` | Command substitution written with backticks instead of `$(...)` |
| `redundant-exit-status-check` | `[ $? -eq 0 ]`, `[[ $? -ne 0 ]]`, or `test "$?" -eq 0` comparing the last exit status with `0` |
| `else-after-return` | An `else` after an `if` branch that always ends in `return` or `exit` |

## Hazard line rules (Hazards)

| Rule | Matches |
| ---- | ------- |
| `missing-strict-mode` | A script with top-level commands but no `set -eu` (POSIX `sh`) or `set -euo pipefail` (Bash and Zsh), from `set`, `setopt` in Zsh, or shebang flags. A function that a top-level command calls counts when its leading `set` or `setopt` commands (before any other command) enable strict mode |
| `unquoted-variable` | An unquoted `$VAR` or `${VAR}` expansion in a command name, argument, or redirection target. In Zsh, only `${=var}`, `$~var`, and command substitutions split, so plain `$VAR` is not reported. A heredoc delimiter is never reported |
| `eval-with-variable` | `eval` with an argument that expands a variable, including a variable inside a command substitution. `eval "$(ssh-agent -s)"` with no variable is not reported |
| `pipe-download-to-shell` | `curl` or `wget` piped into a shell interpreter, or a shell reading `<(curl ...)` |
| `unchecked-cd` | `cd` without `&&`, `\|\|`, an `if` condition, or `set -e`. A `set -e` at the start of a function covers its own `cd` commands from that point |
| `unguarded-rm-rf` | `rm -rf "$VAR/..."` where the variable has no `${VAR:?}` guard |
| `hardcoded-secret` | A non-placeholder literal assigned to a name containing `password`, `secret`, `token`, `api_key`, or a similar word, or a literal with a known secret shape such as an AWS access key, a `ghp_` token, or a private key block. Names ending in `_type`, `_label`, `_cmd`, `_endpoint`, and similar, and values that are URLs, absolute paths, all digits, or contain spaces, are not reported. A token prefix must start at a word boundary, and `sk-` needs a 20-character tail or an `sk-proj-` style shape |
| `read-without-r` | `read` without `-r` |
| `posix-sh-bashism` | Bash-only syntax in a POSIX `sh` script, such as `[[`, `function`, `source`, `((`, arrays, `<<<`, `&>`, or `<(...)` |
| `insecure-tls-download` | `curl -k`, `curl --insecure`, or `wget --no-check-certificate` |

Each Hazard line Finding reports only its path, line, and rule name, never the source text, so `hardcoded-secret` never copies a secret into a report or CI log.
Turn off any rule in `hazards.disabled_rules`:

```jsonc
{
  "hazards": {
    "disabled_rules": ["unquoted-variable", "read-without-r"]
  }
}
```

## Reference ranges

Calibrated Shell Reference ranges come from a stratified panel of 50 current public Shell repositories ([ADR 0031](../adr/0031-shell-reference-ranges.md)), with source `codeint panel of 50 Shell repositories, ADR 0031, codeint 0.6.0`:

| Metric | Reference range |
| ------ | --------------- |
| Erosion | 41.6% to 92.7% |
| Duplication | 0.8% to 75.1% |
| Verbosity | 0.0% to 2.5% |

Import cycles has no Reference range because shell scripts have no imports.
