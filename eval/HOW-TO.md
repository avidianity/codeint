# How to run the Evaluation suite

## Measure the pinned corpus

```sh
cargo build --release --locked -p codeint-cli
cargo run --release --locked -p codeint-eval -- --language python --output-dir docs
```

- Run the command from the repository root.
- `codeint-eval` reads [corpus.json](./corpus.json), measures each selected repository, and writes `evaluation.md` and `evaluation-summary.json` into `--output-dir`.
- It exits with code `0` when every measured repository passes, `1` when one fails, and `2` when the manifest, the `codeint` binary, or the output folder is unusable.

| Option | Default | Meaning |
| ------ | ------- | ------- |
| `--manifest <FILE>` | `eval/corpus.json` | The pinned corpus manifest |
| `--codeint <FILE>` | `target/release/codeint` | The `codeint` binary to measure |
| `--output-dir <DIR>` | `docs` | Where to write `evaluation.md` and `evaluation-summary.json` |
| `--language <PLUGIN>` | every Language plugin | Measure only repositories of this Language plugin; may repeat |
| `--repository <NAME>` | every repository | Measure only the repository with this name, such as `spf13/cobra`; may repeat |
| `--runs <COUNT>` | `3` | How many times to run `codeint check` per repository |
| `--audit-sample-size <COUNT>` | `100` | How many Findings to sample per audit category |
| `--scratch-root <DIR>` | system temporary folder | Create the `codeint-eval-*` scratch folders under this folder |
| `--oracle <PLUGIN=PROGRAM>` | none | Use `PROGRAM` instead of the built-in native-parser oracle for one Language plugin |

## Rules

- Each repository is shallow-cloned at its pinned commit into a new folder under the system temporary folder, or under `--scratch-root`.
  The folder is named `codeint-eval-{process ID}-{random}`, where the process ID is the runner's own.
  The runner exits with code `2` when the scratch folder is inside any of three repositories: the codeint checkout that the binary was built from, the repository that holds the manifest, or the repository that holds the current folder.
  It locates the codeint checkout from the crate's compile-time location, so the manifest and the current folder cannot hide it.
  It deletes the folder when the repository is measured, also when the clone, the measurement, or the oracle fails.
- A run that SIGINT, SIGTERM, or a crash ends cannot delete its folders.
  At the start of every run, the runner sweeps the scratch root for `codeint-eval-{process ID}-*` folders whose process no longer runs, deletes them, and prints `codeint-eval: removed {count} stale scratch folders of ended runs`.
  Liveness comes from `ps -p` on Unix and `tasklist` on Windows; when that program cannot run, the folder stays.
  A folder whose process ID a later process reuses stays until that process ends.
  Folders with other names stay untouched.
- A repository passes when `codeint check` exits `0` and its native-parser recall, when measured, meets the bar.
  A failed clone, a failed measurement, or a failed oracle is recorded as that repository's `error`.
- The runner times `codeint check --format json --all` `--runs` times and reports the median wall time.
  It reads peak resident memory (RSS) from `/usr/bin/time` (`-l` on macOS, `-v` elsewhere); without that program, peak RSS is absent.
- A repository's measured size bucket comes from its Production source lines, with the bands of [ADR 0028](../docs/adr/0028-stratified-reference-panels.md).
  The `bucket` field of the manifest is the declared bucket; the report shows both.
- Each corpus entry names a public repository, its URL, and an exact 40-digit commit SHA.
  Never commit third-party source code; the manifest holds only references.

## Native-parser oracles

| Language plugin | Oracle | Runtime |
| --------------- | ------ | ------- |
| `python` | Standard library `ast` | `python3` 3.12 or newer |
| `go` | Standard library `go/parser` | `go` 1.24 or newer |
| `php` | `PhpToken::tokenize` with `TOKEN_PARSE` | `php` 8.1 or newer |
| `dart` | `package:analyzer` 14.5.0 | `dart` 3.11 or newer, and network access for `dart pub get` |

The Dart minimum is 3.11 because `analyzer` 14.5.0 requires it.
The newest `analyzer` that admits Dart 3.0 is 6.4.1; it fails with "Expected an identifier." on dot shorthands and null-aware list elements, so it would mark current Dart files as oracle-unparsable.

- The oracle sources live in [oracles/](./oracles/).
  The runner writes them into a scratch folder outside the checkout, and builds the Go and Dart oracles there.
  `dart pub get` uses a `PUB_CACHE` inside that scratch folder, so nothing outlives the run.
- When a runtime is missing or too old, or an oracle does not build, the language's recall status is `skipped` with the reason.
- Recall compares each file that the oracle parses with the codeint Language plugin's model of the same file:
  functions match by first and last line, decision points count up to the oracle's count per matched function, and static imports match by specifier.
  Functions that share a first and last line pair by decision points, highest first, so the order of the two lists never changes the score.
  The report shows each percentage cut off, not rounded, at two decimals, so a value below the bar never shows as the bar.
- The bar is 98% recall on functions, decision points, and static imports, with no file that the oracle parses and codeint cannot.
  Files that the oracle cannot parse leave the recall counts and are listed as oracle-unparsable.
  An oracle parses the file bytes, so a file that is valid in its own encoding, such as a Python file with a `# -*- coding: latin-1 -*-` line, is parsed and then counted as unmeasured when `codeint` cannot read it as UTF-8.
  The Go and Dart parsers accept only UTF-8, so a file in another encoding is oracle-unparsable for them.
- A file that the oracle parses and codeint did not measure, such as a file that is not UTF-8 or that is over a limit, counts every oracle function, decision point, and import in it as a miss.
  The report lists these files as unmeasured.
- The measured files are exactly the files that `codeint` measures: the runner asks the `codeint-git` crate, which `codeint check` uses, for the working-tree files of the clone.
  A tracked symlink, a submodule, or any other tracked source file that `codeint` does not measure is still sent to the oracle, then counted as unmeasured and left out of the audit sample.

An oracle program receives one argument: a file that lists repository-relative paths, each ended by a NUL byte, so a path with a newline survives.
It runs in the root of the clone and prints one JSON line per path:

```json
{"path":"app/calc.py","functions":[[4,8,2]],"imports":["os"]}
{"path":"app/broken.py","error":"invalid syntax"}
```

Each function is `[first line, last line, decision points]`.

## Human audit sample

- The runner samples up to `--audit-sample-size` Findings in each of three categories: Source sets, Flagged lines, and Orphan files.
- The seed comes from the sorted pinned commits, so the same corpus always yields the same sample.
- Source set items use the Language plugin's Source set with the core Generated and Vendored rules; Framework profile paths are not applied.
