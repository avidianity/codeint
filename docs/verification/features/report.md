# Feature map: `codeint report`

Verifies re-rendering saved `codeint check --format json` and `codeint diff --format json` payloads into `toon`, `json`, `sarif`, `markdown`, and `text`, reading from standard input (`-`), `--all` text expansion, `--output`, and operational errors.
See [codeint report](../../reference.md#codeint-report) for the command specification.

## Preconditions

Run Launch and Doctor from [docs/verification/README.md](../README.md):

```sh
export CODEINT_VERIFY_OWNER="$(id -un):$$"
eval "$(docs/verification/verify.sh launch)"
docs/verification/verify.sh doctor "$RUN_DIR"
```

## States covered

| Case | State | Expected exit | Expected visible result and authoritative effect |
| ---- | ----- | ------------- | ------------------------------------------------ |
| `check_to_sarif_output` | Saved `check` JSON re-rendered with `--format sarif --output report.sarif` | `0` | Standard output is empty; `report.sarif` is valid SARIF `2.1.0` with driver `codeint` |
| `check_to_markdown` | Saved `check` JSON re-rendered with `--format markdown` | `0` | Standard output contains `codeint report` and the Metric table |
| `check_to_text_stdin` | Saved `check` JSON piped via `-` with `--format text` | `0` | Standard output contains concise monochrome text summary (`codeint check: PASS`) and aligned Metric rows |
| `check_to_text_all` | Saved `check` JSON re-rendered with `--format text --all` | `0` | Standard output matches `codeint check --all --format text` concise monochrome output without ANSI escapes |
| `diff_to_toon` | Saved `diff` JSON re-rendered with `--format toon` | `0` | Standard output contains non-empty TOON diff report |
| `diff_to_json` | Saved `diff` JSON re-rendered with `--format json` | `0` | Valid JSON diff payload matching the saved report |
| `missing_input` | Input file that does not exist | `2` | Standard error reports the unreadable file |
| `malformed_json` | Input file holding invalid JSON | `2` | Standard error reports invalid JSON |
| `unsupported_schema` | Input file with `{"schema_version": 999}` | `2` | Standard error rejects the unsupported schema version |

## Drive and Assert

```sh
docs/verification/verify.sh drive "$RUN_DIR" report
docs/verification/verify.sh assert "$RUN_DIR" report
```

Assert checks each re-rendered format, standard-input reading, `--output` with empty stdout, error exits, and `FIXTURE_REPO` cleanliness.

## Evidence and Cleanup

Evidence files are stored under `$EVIDENCE_DIR/report/` and `$EVIDENCE_DIR/verdict.json`.
Clean up the isolated run workspace while keeping evidence:

```sh
docs/verification/verify.sh cleanup "$RUN_DIR"
```
