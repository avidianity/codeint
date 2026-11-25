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
| `--oracle <PLUGIN=PROGRAM>` | none | Use `PROGRAM` instead of the built-in native-parser oracle for one Language plugin |

## Rules

- Each repository is shallow-cloned at its pinned commit into a new folder under the system temporary folder, named `codeint-eval-*`.
  The runner refuses a temporary folder inside the codeint checkout.
  It deletes the folder when the repository is measured, also when the clone, the measurement, or the oracle fails.
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

- The oracle sources live in [oracles/](./oracles/).
  The runner writes them into a scratch folder outside the checkout, and builds the Go and Dart oracles there.
- When a runtime is missing or too old, or an oracle does not build, the language's recall status is `skipped` with the reason.
- Recall compares each file that the oracle parses with the codeint Language plugin's model of the same file:
  functions match by first and last line, decision points count up to the oracle's count per matched function, and static imports match by specifier.
- The bar is 98% recall on functions, decision points, and static imports, with no file that the oracle parses and codeint cannot.
  Files that the oracle cannot parse leave the recall counts and are listed as oracle-unparsable.
- A file that the oracle parses and codeint did not measure, such as a file that is not UTF-8 or that is over a limit, counts every oracle function, decision point, and import in it as a miss.
  The report lists these files as unmeasured.

An oracle program receives one argument: a file that lists one repository-relative path per line.
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
