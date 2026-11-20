# Runtime verification contract

This page defines how to launch, check, drive, assert, record, and clean up a live runtime verification of `codeint`.
Every command below is executable from the root of a `codeint` checkout.
See [docs/reference.md](../reference.md) for command options, exit codes, report formats, and `codeint.json` keys.
See [features/README.md](./features/README.md) for the mapped user-visible features.

## Concurrency

Multiple copies of this verification can run at the same time.
Each run generates a unique `run_id` and creates isolated directories under `${TMPDIR:-/tmp}`:

- a run workspace (`codeint-verify-run-<run_id>`) holding fixture Git repositories and `CODEINT_CACHE_DIR`;
- an evidence directory (`codeint-verify-evidence-<run_id>`) holding logs, outputs, and `verdict.json`.

`codeint` binds no network port and writes no shared state when `CODEINT_CACHE_DIR` points to the run's isolated cache folder.

## Operations

Set `CODEINT_VERIFY_OWNER` once in your shell when running operations step by step so Doctor recognizes your session:

```sh
export CODEINT_VERIFY_OWNER="$(id -un):$$"
```

### Launch

Build `codeint` from the current checkout, create isolated fixture repositories and cache directory, and write `manifest.env` with the exact `git rev-parse HEAD` revision and SHA-256 hash of `target/debug/codeint`:

```sh
eval "$(docs/verification/verify.sh launch)"
```

`launch` prints `RUN_DIR=...` and `EVIDENCE_DIR=...` to standard output.
Inside `RUN_DIR`, it creates:

- `repo/`: a deterministic three-commit Git repository (`main` with two commits and `feature` with a third commit) containing a Flagged line (`src/sign.ts`), a declaration-only file (`src/types.d.ts`), an unparsable file (`src/broken.ts`), an eroded function in history (`src/parse.ts`), and a clean file (`src/total.ts`);
- `init-repo/`: a clean single-commit Git repository with `src/app.ts` and no `codeint.json`;
- `empty-repo/`: an initialized Git repository with an unborn `HEAD` and no commits;
- `non-repo/`: a plain folder outside any Git repository;
- `cache/`: an isolated `CODEINT_CACHE_DIR`;
- `manifest.env`: `RUN_ID`, `REPO_ROOT`, `BIN_PATH`, `BUILD_REV`, `BIN_SHA256`, `OWNER_CHECKOUT`, `OWNER_ID`, and `FIXTURE_HEAD`.

### Doctor

Verify that the launched copy is built from the current checkout, matches `HEAD`, belongs to this checkout and owner, and is healthy:

```sh
docs/verification/verify.sh doctor "$RUN_DIR"
```

Doctor is read-only.
It exits `0` only when all checks pass:

- `OWNER_CHECKOUT` matches `git rev-parse --show-toplevel`;
- `OWNER_ID` matches `CODEINT_VERIFY_OWNER` (or `$(id -un):$PPID`);
- `BUILD_REV` matches `git rev-parse HEAD`;
- `target/debug/codeint` exists, is executable, and matches `BIN_SHA256`;
- `target/debug/codeint --version` exits `0`;
- `FIXTURE_REPO` is a valid Git work tree and `CACHE_DIR` is writable.

Doctor exits `1` and refuses the run when the binary is stale, `HEAD` moved, or the run directory belongs to another checkout or session.

### Drive

Run the real `codeint` binary against the isolated fixture repositories for all mapped features or one named feature (`check`, `diff`, `trend`, or `init`):

```sh
docs/verification/verify.sh drive "$RUN_DIR" all
```

To drive one feature:

```sh
docs/verification/verify.sh drive "$RUN_DIR" check
```

Every `codeint` invocation runs with `CODEINT_CACHE_DIR="$CACHE_DIR"`.
Scenarios that mutate `codeint.json` or working-tree files copy the fixture repo into a per-scenario work folder inside `RUN_DIR` so `FIXTURE_REPO` stays untouched.

### Assert

Run machine-checkable assertions on every recorded exit code, stdout/stderr shape, output file, cache entry, and repository state:

```sh
docs/verification/verify.sh assert "$RUN_DIR" all
```

Assert checks both visible results and authoritative effects:

- exit codes `0` (pass), `1` (over Budget or Regression), and `2` (operational error);
- TOON, JSON, SARIF 2.1.0, and Markdown output shapes;
- `--output` file creation with empty stdout;
- `codeint.json` written by `init` is valid JSON and remains unchanged after a refused second `init`;
- `CODEINT_CACHE_DIR` holds at least one cache entry after `diff` or `trend`;
- `FIXTURE_REPO` has empty `git status --porcelain` and unchanged `HEAD`.

Assert writes `verdict.json` to `EVIDENCE_DIR` and exits `0` on `PASS` or `1` on `FAIL`.

### Evidence

All evidence is written to `EVIDENCE_DIR` (`/tmp/codeint-verify-evidence-<run_id>` or `$TMPDIR/codeint-verify-evidence-<run_id>`), never inside the tracked repository.
Each driven case stores:

- `<feature>/<case>.cmd`: exact command line;
- `<feature>/<case>.exit`: numeric exit code;
- `<feature>/<case>.stdout`: captured standard output;
- `<feature>/<case>.stderr`: captured standard error;
- generated report files (`report.md`, `diff.md`, `diff.sarif`, `trend.md`, `codeint.before.json`, `codeint.after.json`);
- `verdict.json`: `verdict`, `build_rev`, `run_id`, `feature`, `checks_passed`, `checks_failed`, `failures`, and `reproduce`.

All fixture authors use synthetic identity (`Verify Author <verify@example.invalid>`) and no secrets or personal data appear in evidence files.

### Cleanup

Remove the isolated run workspace (`FIXTURE_REPO`, scratch copies, and `CACHE_DIR`) while keeping `EVIDENCE_DIR`:

```sh
docs/verification/verify.sh cleanup "$RUN_DIR"
```

Cleanup is idempotent.
Running Cleanup a second time exits `0` and leaves `EVIDENCE_DIR` intact.

## Single-command full run

Run Launch, Doctor, Drive, Assert, and Cleanup in one command:

```sh
docs/verification/verify.sh all
```

Pass a feature name (`check`, `diff`, `trend`, or `init`) to verify one Feature map:

```sh
docs/verification/verify.sh all check
```
