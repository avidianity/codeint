# Feature map: `codeint guard`

Verifies Import boundary evaluation for existing and planned file paths, internal zone allow and deny rules, external-package rules, unconfigured repositories, `text`, `toon`, and `json` formats, and operational errors.
See [codeint guard](../../reference.md#codeint-guard) for the command specification.

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
| `unconfigured_json` | Repo with no `boundaries` config, `codeint guard src/sign.ts --format json` | `0` | JSON reports `configured: false` and `exists: true` |
| `configured_existing_json` | Repo with `domain` and `ui` zones, `codeint guard src/domain/order.ts --format json` | `0` | JSON reports `configured: true`, `exists: true`, `zone: "domain"`, `denied_targets` containing `ui`, and `external_imports.directive == "deny_external"` |
| `configured_planned_toon` | Non-existent planned path `src/ui/planned.ts` with `--format toon` | `0` | TOON output reports `exists: false`, `zone: ui`, and `domain` in allowed targets |
| `configured_text` | `codeint guard src/domain/order.ts --format text` | `0` | Text output names the `domain` zone and `ui` |
| `outside_repo` | Path `../outside.ts` escaping the Git repository | `2` | Standard error reports the path is outside the Git repository |
| `non_repo` | Folder outside a Git repository | `2` | Standard error reports the folder is not inside a Git repository |

## Drive and Assert

```sh
docs/verification/verify.sh drive "$RUN_DIR" guard
docs/verification/verify.sh assert "$RUN_DIR" guard
```

Assert checks unconfigured, existing-file, and planned-file evaluations across JSON, TOON, and text formats, error exits, and `FIXTURE_REPO` cleanliness.

## Evidence and Cleanup

Evidence files are stored under `$EVIDENCE_DIR/guard/` and `$EVIDENCE_DIR/verdict.json`.
Clean up the isolated run workspace while keeping evidence:

```sh
docs/verification/verify.sh cleanup "$RUN_DIR"
```
