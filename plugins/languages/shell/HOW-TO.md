# How to use and change the Shell Language plugin

## Call the plugin

```rust
use codeint_lang_shell::{
    SHELL_FLAGGED_RULES, SHELL_HAZARD_RULES, Shell, ShellDialect, detect_dialect,
};
use codeint_model::{LanguagePlugin, RepoPath};

let plugin = Shell;
let path = RepoPath::new("scripts/deploy.sh")?;
let source = "#!/usr/bin/env bash\nset -euo pipefail\necho ok\n";
let dialect: ShellDialect = detect_dialect(path.as_str(), source);
let model = plugin.lower(&path, source, None)?;
```

- `plugins/languages/shell` (`codeint-lang-shell`, plugin name `shell`) is a built-in Language plugin for `.sh`, `.bash`, and `.zsh` files under `#![forbid(unsafe_code)]`.
- It also accepts known generated Autotools filenames (such as `configure`, `config.guess`, `config.sub`, `install-sh`, `ltmain.sh`, and `missing`) so they lower into the `Generated` Source set.
- `has_imports` is `false`, `has_hazard_rules` is `true`, `has_scope_analysis` is `false`, `known_dialects` is empty, and `known_framework_profiles` is empty.
- `lower` ignores the optional `dialect` argument and returns `LowerError::UnsupportedFile` for unsupported paths.

## Dialect detection and Source sets

- `detect_dialect` (`src/dialect.rs`) determines `ShellDialect::{PosixSh, Bash, Zsh}` per file from the `#!` shebang first (unwrapping `env` and `busybox` launchers and recognizing versioned `bash`/`zsh` names and `sh`/`dash`/`ash`/`posix-sh`), then falls back to the file extension (`.bash`, `.zsh`, otherwise `PosixSh`).
- `src/source_set.rs` classifies each file in this priority order:
  1. `Vendored` when any folder segment is `vendor` or `third_party`.
  2. `Generated` when the filename is in the generated Autotools list or a header comment matches a generated-file marker (`code generated`, `do not modify by hand`, `this file is generated`, or the shared markers in `codeint-model`).
  3. `Test` when any folder segment is `test`, `tests`, or `spec`, or when the stem starts with `test_`, ends with `_test`, or ends with `.test`.
  4. `Production` otherwise.

## Functions, tokens, and Flagged lines

- `src/functions.rs` extracts shell function definitions and computes decision points and nesting-weighted cognitive complexity (`src/functions/cognitive.rs`).
- `src/lib.rs` emits Duplication tokens from the lexed stream and recursively flattens command-substitution `sub_tokens`, skipping newline tokens.
- `src/flagged.rs` exports `SHELL_FLAGGED_RULES` and `flagged_line_rules()` with four Flagged line rules:
  - `useless-cat`
  - `legacy-backticks`
  - `redundant-exit-status-check`
  - `else-after-return`

## Hazard line rules

- `src/hazards.rs` exports `SHELL_HAZARD_RULES` and `hazard_line_rules()` with ten Hazard line rules:
  - `missing-strict-mode`
  - `unquoted-variable`
  - `eval-with-variable`
  - `pipe-download-to-shell`
  - `unchecked-cd`
  - `unguarded-rm-rf`
  - `hardcoded-secret`
  - `read-without-r`
  - `posix-sh-bashism`
  - `insecure-tls-download`
- `missing-strict-mode` flags the first code line of a script that has top-level executable code without strict mode (`src/hazards/strict_mode.rs`).
- `posix-sh-bashism` (`src/hazards/bashisms.rs`) runs only when `detect_dialect` resolves `ShellDialect::PosixSh`.

## Reference ranges

- `reference_ranges` returns the 10th to 90th percentile bands from the stratified 100-repository Shell panel under [ADR 0035](../../../docs/adr/0035-stratified-100-repository-reference-panels.md).
- Its source label is `codeint stratified panel of 100 Shell repositories, ADR 0035, codeint 0.8.0`.
