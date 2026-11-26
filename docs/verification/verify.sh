#!/usr/bin/env bash
set -euo pipefail

sha256_file() {
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    sha256sum "$1" | awk '{print $1}'
  fi
}

owner_identity() {
  if [ -n "${CODEINT_VERIFY_OWNER:-}" ]; then
    printf '%s' "$CODEINT_VERIFY_OWNER"
  else
    printf '%s:%s' "$(id -un)" "$PPID"
  fi
}

git_commit_deterministic() {
  local repo="$1"
  local date="$2"
  local msg="$3"
  GIT_AUTHOR_NAME="Verify Author" \
  GIT_AUTHOR_EMAIL="verify@example.invalid" \
  GIT_COMMITTER_NAME="Verify Author" \
  GIT_COMMITTER_EMAIL="verify@example.invalid" \
  GIT_AUTHOR_DATE="$date" \
  GIT_COMMITTER_DATE="$date" \
  git -C "$repo" commit -q -m "$msg"
}

write_eroded_ts() {
  local target="$1"
  local fn_name="$2"
  local branches="$3"
  {
    printf 'export function %s(code: number): number {\n' "$fn_name"
    printf '  let acc = 0;\n'
    local i=1
    while [ "$i" -le "$branches" ]; do
      printf '  if (code === %d) { acc += %d; }\n' "$i" "$i"
      i=$((i + 1))
    done
    printf '  return acc;\n'
    printf '}\n'
  } > "$target"
}

cmd_launch() {
  local run_id=""
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --run-id)
        run_id="$2"
        shift 2
        ;;
      *)
        printf 'unknown launch option: %s\n' "$1" >&2
        exit 2
        ;;
    esac
  done

  if [ -z "$run_id" ]; then
    run_id="$(date -u +%Y%m%dT%H%M%SZ)-$$-$RANDOM"
  fi

  local repo_root
  repo_root="$(git rev-parse --show-toplevel)"
  cargo build --locked --manifest-path "$repo_root/Cargo.toml" >&2

  local bin_path="$repo_root/target/debug/codeint"
  local build_rev
  build_rev="$(git -C "$repo_root" rev-parse HEAD)"
  local bin_sha256
  bin_sha256="$(sha256_file "$bin_path")"
  local owner_id
  owner_id="$(owner_identity)"

  local base_tmp="${TMPDIR:-/tmp}"
  base_tmp="${base_tmp%/}"
  local run_dir="${base_tmp}/codeint-verify-run-${run_id}"
  local evidence_dir="${base_tmp}/codeint-verify-evidence-${run_id}"

  if [ -e "$run_dir" ]; then
    printf 'run directory already exists: %s\n' "$run_dir" >&2
    exit 2
  fi

  local fixture_repo="$run_dir/repo"
  local init_repo="$run_dir/init-repo"
  local empty_repo="$run_dir/empty-repo"
  local non_repo="$run_dir/non-repo"
  local cache_dir="$run_dir/cache"

  mkdir -p "$fixture_repo/src" "$init_repo/src" "$empty_repo" "$non_repo" "$cache_dir" "$evidence_dir"

  git -C "$fixture_repo" init -b main -q
  cat > "$fixture_repo/src/sign.ts" <<'EOF'
export function sign(n: number): number {
  if (n > 0) {
    return 1;
  } else {
    return -1;
  }
}
EOF
  cat > "$fixture_repo/src/types.d.ts" <<'EOF'
export interface Item {
  id: string;
}
EOF
  printf 'export const broken = (\n' > "$fixture_repo/src/broken.ts"
  git -C "$fixture_repo" add .
  git_commit_deterministic "$fixture_repo" "2026-11-02T12:00:00Z" "Add sign"

  write_eroded_ts "$fixture_repo/src/parse.ts" "parse" 11
  git -C "$fixture_repo" add .
  git_commit_deterministic "$fixture_repo" "2026-11-09T12:00:00Z" "Add parse"

  git -C "$fixture_repo" checkout -b feature -q
  cat > "$fixture_repo/src/parse.ts" <<'EOF'
export function parse(code: number): number {
  return code + 1;
}
EOF
  cat > "$fixture_repo/src/total.ts" <<'EOF'
export function total(values: number[]): number {
  let sum = 0;
  for (const v of values) {
    sum += v;
  }
  return sum;
}

export function average(values: number[]): number {
  if (values.length === 0) {
    return 0;
  }
  return total(values) / values.length;
}

export function clamp(value: number, low: number, high: number): number {
  const lower = Math.max(value, low);
  return Math.min(lower, high);
}
EOF
  git -C "$fixture_repo" add .
  git_commit_deterministic "$fixture_repo" "2026-11-16T12:00:00Z" "Simplify parse"

  local fixture_head
  fixture_head="$(git -C "$fixture_repo" rev-parse HEAD)"

  git -C "$init_repo" init -b main -q
  printf 'export const app = 1;\n' > "$init_repo/src/app.ts"
  git -C "$init_repo" add .
  git_commit_deterministic "$init_repo" "2026-11-16T12:00:00Z" "Initial app"

  git -C "$empty_repo" init -b main -q
  printf 'export const x = 1;\n' > "$non_repo/index.ts"

  cat > "$run_dir/manifest.env" <<EOF
RUN_ID='${run_id}'
REPO_ROOT='${repo_root}'
BIN_PATH='${bin_path}'
BUILD_REV='${build_rev}'
BIN_SHA256='${bin_sha256}'
OWNER_CHECKOUT='${repo_root}'
OWNER_ID='${owner_id}'
RUN_DIR='${run_dir}'
FIXTURE_REPO='${fixture_repo}'
INIT_REPO='${init_repo}'
EMPTY_REPO='${empty_repo}'
NON_REPO='${non_repo}'
CACHE_DIR='${cache_dir}'
EVIDENCE_DIR='${evidence_dir}'
FIXTURE_HEAD='${fixture_head}'
EOF

  printf 'RUN_DIR=%s\n' "$run_dir"
  printf 'EVIDENCE_DIR=%s\n' "$evidence_dir"
}

load_manifest() {
  local run_dir="$1"
  local manifest="$run_dir/manifest.env"
  if [ ! -f "$manifest" ]; then
    printf 'doctor: manifest not found at %s\n' "$manifest" >&2
    exit 1
  fi
  set -a
  source "$manifest"
  set +a
}

cmd_doctor() {
  local run_dir="${1:?usage: verify.sh doctor <run-dir>}"
  load_manifest "$run_dir"

  local current_root
  current_root="$(git rev-parse --show-toplevel)"
  local current_rev
  current_rev="$(git -C "$current_root" rev-parse HEAD)"
  local current_owner
  current_owner="$(owner_identity)"

  if [ "$OWNER_CHECKOUT" != "$current_root" ]; then
    printf 'doctor: foreign checkout: manifest=%s current=%s\n' "$OWNER_CHECKOUT" "$current_root" >&2
    exit 1
  fi

  if [ "$OWNER_ID" != "$current_owner" ]; then
    printf 'doctor: foreign owner: manifest=%s current=%s\n' "$OWNER_ID" "$current_owner" >&2
    exit 1
  fi

  if [ "$BUILD_REV" != "$current_rev" ]; then
    printf 'doctor: stale revision: manifest=%s current=%s\n' "$BUILD_REV" "$current_rev" >&2
    exit 1
  fi

  if [ ! -x "$BIN_PATH" ]; then
    printf 'doctor: binary missing or not executable: %s\n' "$BIN_PATH" >&2
    exit 1
  fi

  local current_sha
  current_sha="$(sha256_file "$BIN_PATH")"
  if [ "$BIN_SHA256" != "$current_sha" ]; then
    printf 'doctor: stale binary hash: manifest=%s current=%s\n' "$BIN_SHA256" "$current_sha" >&2
    exit 1
  fi

  local version_out
  if ! version_out="$("$BIN_PATH" --version 2>&1)"; then
    printf 'doctor: --version failed: %s\n' "$version_out" >&2
    exit 1
  fi

  if [ ! -d "$FIXTURE_REPO/.git" ] || ! git -C "$FIXTURE_REPO" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    printf 'doctor: unhealthy fixture repo: %s\n' "$FIXTURE_REPO" >&2
    exit 1
  fi

  if [ ! -d "$CACHE_DIR" ] || [ ! -w "$CACHE_DIR" ]; then
    printf 'doctor: unhealthy cache dir: %s\n' "$CACHE_DIR" >&2
    exit 1
  fi

  printf 'doctor: OK rev=%s version=%s run_id=%s\n' "$BUILD_REV" "$version_out" "$RUN_ID"
}

run_case() {
  local workdir="$1"
  local out_prefix="$2"
  shift 2
  local exit_code=0
  printf '%s\n' "$*" > "${out_prefix}.cmd"
  (
    cd "$workdir"
    CODEINT_CACHE_DIR="$CACHE_DIR" "$@"
  ) > "${out_prefix}.stdout" 2> "${out_prefix}.stderr" || exit_code=$?
  printf '%s\n' "$exit_code" > "${out_prefix}.exit"
}

drive_check() {
  local dir="$EVIDENCE_DIR/check"
  mkdir -p "$dir"

  run_case "$FIXTURE_REPO" "$dir/golden_toon" "$BIN_PATH" check
  run_case "$FIXTURE_REPO" "$dir/golden_json" "$BIN_PATH" check --format json
  run_case "$FIXTURE_REPO" "$dir/golden_sarif" "$BIN_PATH" check --format sarif
  run_case "$FIXTURE_REPO" "$dir/golden_md_output" "$BIN_PATH" check --format markdown --output "$dir/report.md"
  run_case "$FIXTURE_REPO" "$dir/golden_all" "$BIN_PATH" check --all --format json

  local work_budget="$RUN_DIR/work-check-budget"
  rm -rf "$work_budget"
  cp -R "$FIXTURE_REPO" "$work_budget"
  printf '{"budgets":{"production":{"verbosity":0}}}\n' > "$work_budget/codeint.json"
  run_case "$work_budget" "$dir/over_budget" "$BIN_PATH" check --format json

  local work_badcfg="$RUN_DIR/work-check-badcfg"
  rm -rf "$work_badcfg"
  cp -R "$FIXTURE_REPO" "$work_badcfg"
  printf '{"budget":{}}\n' > "$work_badcfg/codeint.json"
  run_case "$work_badcfg" "$dir/invalid_config" "$BIN_PATH" check

  local work_manifest="$RUN_DIR/work-check-manifest"
  rm -rf "$work_manifest"
  cp -R "$FIXTURE_REPO" "$work_manifest"
  mkdir -p "$work_manifest/fixtures"
  : > "$work_manifest/fixtures/go.mod"
  printf 'package fixtures\n\nfunc One() int {\n\treturn 1\n}\n' > "$work_manifest/fixtures/one.go"
  run_case "$work_manifest" "$dir/invalid_manifest" "$BIN_PATH" check --format json

  run_case "$FIXTURE_REPO" "$dir/missing_path" "$BIN_PATH" check missing
  run_case "$NON_REPO" "$dir/non_repo" "$BIN_PATH" check
}

drive_diff() {
  local dir="$EVIDENCE_DIR/diff"
  mkdir -p "$dir"

  run_case "$FIXTURE_REPO" "$dir/no_regression_1" "$BIN_PATH" diff main --head feature --format json
  run_case "$FIXTURE_REPO" "$dir/no_regression_2" "$BIN_PATH" diff main --head feature --format json
  run_case "$FIXTURE_REPO" "$dir/no_regression_md" "$BIN_PATH" diff main --head feature --format markdown --output "$dir/diff.md"

  local work_reg="$RUN_DIR/work-diff-regression"
  rm -rf "$work_reg"
  cp -R "$FIXTURE_REPO" "$work_reg"
  write_eroded_ts "$work_reg/src/route.ts" "route" 16
  run_case "$work_reg" "$dir/regression_json" "$BIN_PATH" diff main --format json
  run_case "$work_reg" "$dir/regression_sarif" "$BIN_PATH" diff main --format sarif --output "$dir/diff.sarif"

  run_case "$FIXTURE_REPO" "$dir/bad_ref" "$BIN_PATH" diff no-such-branch
  run_case "$EMPTY_REPO" "$dir/unborn_head" "$BIN_PATH" diff main
}

drive_trend() {
  local dir="$EVIDENCE_DIR/trend"
  mkdir -p "$dir"

  run_case "$FIXTURE_REPO" "$dir/history_json" "$BIN_PATH" trend --format json
  run_case "$FIXTURE_REPO" "$dir/history_toon" "$BIN_PATH" trend --format toon
  run_case "$FIXTURE_REPO" "$dir/since_json" "$BIN_PATH" trend --since 2026-11-05 --format json
  run_case "$FIXTURE_REPO" "$dir/history_md" "$BIN_PATH" trend --format markdown --output "$dir/trend.md"
  run_case "$FIXTURE_REPO" "$dir/bad_since" "$BIN_PATH" trend --since 2026-02-30
  run_case "$EMPTY_REPO" "$dir/unborn_head" "$BIN_PATH" trend
}

drive_init() {
  local dir="$EVIDENCE_DIR/init"
  mkdir -p "$dir"

  local work_init="$RUN_DIR/work-init"
  rm -rf "$work_init"
  cp -R "$INIT_REPO" "$work_init"

  run_case "$work_init" "$dir/fresh" "$BIN_PATH" init
  if [ -f "$work_init/codeint.jsonc" ]; then
    cp "$work_init/codeint.jsonc" "$dir/codeint.before.json"
    sha256_file "$work_init/codeint.jsonc" > "$dir/codeint.before.sha256"
  fi

  run_case "$work_init" "$dir/existing" "$BIN_PATH" init
  if [ -f "$work_init/codeint.jsonc" ]; then
    cp "$work_init/codeint.jsonc" "$dir/codeint.after.json"
    sha256_file "$work_init/codeint.jsonc" > "$dir/codeint.after.sha256"
  fi

  run_case "$NON_REPO" "$dir/non_repo" "$BIN_PATH" init

  local work_adopt="$RUN_DIR/work-init-adopt-fresh"
  rm -rf "$work_adopt"
  cp -R "$INIT_REPO" "$work_adopt"
  write_eroded_ts "$work_adopt/src/legacy.ts" "legacy" 16
  git -C "$work_adopt" add .
  git_commit_deterministic "$work_adopt" "2026-11-17T12:00:00Z" "Add legacy"
  run_case "$work_adopt" "$dir/adopt_fresh" "$BIN_PATH" init --adopt
  if [ -f "$work_adopt/codeint.jsonc" ]; then
    cp "$work_adopt/codeint.jsonc" "$dir/adopt_fresh.config.json"
  fi
  if [ -f "$work_adopt/.codeint/ratchet.json" ]; then
    cp "$work_adopt/.codeint/ratchet.json" "$dir/adopt_fresh.ratchet.json"
  fi
  if [ -f "$work_adopt/.codeint/baseline.json" ]; then
    cp "$work_adopt/.codeint/baseline.json" "$dir/adopt_fresh.baseline.json"
  fi
  run_case "$work_adopt" "$dir/adopt_check_clean" "$BIN_PATH" check --format json
  write_eroded_ts "$work_adopt/src/fresh.ts" "fresh" 16
  printf 'export const legacy = 1;\n' > "$work_adopt/src/legacy.ts"
  run_case "$work_adopt" "$dir/adopt_check_new" "$BIN_PATH" check --format json

  local work_adopt_rollback="$RUN_DIR/work-init-adopt-rollback"
  rm -rf "$work_adopt_rollback"
  cp -R "$INIT_REPO" "$work_adopt_rollback"
  write_eroded_ts "$work_adopt_rollback/src/legacy.ts" "legacy" 16
  printf 'not a folder\n' > "$work_adopt_rollback/.codeint"
  run_case "$work_adopt_rollback" "$dir/adopt_rollback" "$BIN_PATH" init --adopt
  find "$work_adopt_rollback" -path "$work_adopt_rollback/.git" -prune -o -type f -print | sed "s|^$work_adopt_rollback/||" | sort > "$dir/adopt_rollback.files"

  local work_adopt_cfg="$RUN_DIR/work-init-adopt-config"
  rm -rf "$work_adopt_cfg"
  cp -R "$INIT_REPO" "$work_adopt_cfg"
  printf '{"budgets":{}}\n' > "$work_adopt_cfg/codeint.jsonc"
  sha256_file "$work_adopt_cfg/codeint.jsonc" > "$dir/adopt_existing_config.before.sha256"
  run_case "$work_adopt_cfg" "$dir/adopt_existing_config" "$BIN_PATH" init --adopt
  sha256_file "$work_adopt_cfg/codeint.jsonc" > "$dir/adopt_existing_config.after.sha256"
  find "$work_adopt_cfg" -path "$work_adopt_cfg/.git" -prune -o -type f -print | sed "s|^$work_adopt_cfg/||" | sort > "$dir/adopt_existing_config.files"

  local work_adopt_state="$RUN_DIR/work-init-adopt-state"
  rm -rf "$work_adopt_state"
  cp -R "$INIT_REPO" "$work_adopt_state"
  mkdir -p "$work_adopt_state/.codeint"
  printf '{"version":1,"findings":[]}\n' > "$work_adopt_state/.codeint/baseline.json"
  sha256_file "$work_adopt_state/.codeint/baseline.json" > "$dir/adopt_existing_state.before.sha256"
  run_case "$work_adopt_state" "$dir/adopt_existing_state" "$BIN_PATH" init --adopt
  sha256_file "$work_adopt_state/.codeint/baseline.json" > "$dir/adopt_existing_state.after.sha256"
  find "$work_adopt_state" -path "$work_adopt_state/.git" -prune -o -type f -print | sed "s|^$work_adopt_state/||" | sort > "$dir/adopt_existing_state.files"
}

cmd_drive() {
  local run_dir="${1:?usage: verify.sh drive <run-dir> [all|check|diff|trend|init]}"
  local feature="${2:-all}"
  load_manifest "$run_dir"

  case "$feature" in
    all)
      drive_check
      drive_diff
      drive_trend
      drive_init
      ;;
    check) drive_check ;;
    diff) drive_diff ;;
    trend) drive_trend ;;
    init) drive_init ;;
    *)
      printf 'unknown feature: %s\n' "$feature" >&2
      exit 2
      ;;
  esac
  printf 'drive: OK feature=%s evidence=%s\n' "$feature" "$EVIDENCE_DIR"
}

cmd_assert() {
  local run_dir="${1:?usage: verify.sh assert <run-dir> [all|check|diff|trend|init]}"
  local feature="${2:-all}"
  load_manifest "$run_dir"

  local git_status
  git_status="$(git -C "$FIXTURE_REPO" status --porcelain)"
  local head_now
  head_now="$(git -C "$FIXTURE_REPO" rev-parse HEAD)"
  local cache_count
  cache_count="$(find "$CACHE_DIR" -type f | wc -l | tr -d ' ')"

  python3 - "$EVIDENCE_DIR" "$feature" "$BUILD_REV" "$RUN_ID" "$FIXTURE_HEAD" "$head_now" "$git_status" "$cache_count" <<'PY'
import json
import os
import sys

evidence_dir, feature, build_rev, run_id, expected_head, actual_head, git_status, cache_count = sys.argv[1:9]
failures = []
passed = []

def check(name, cond, detail=""):
    if cond:
        passed.append(name)
    else:
        failures.append({"check": name, "detail": detail})

def read_text(rel):
    p = os.path.join(evidence_dir, rel)
    if not os.path.exists(p):
        return None
    with open(p, "r", encoding="utf-8") as f:
        return f.read()

def read_exit(rel):
    t = read_text(rel + ".exit")
    return int(t.strip()) if t is not None else None

def read_json(rel):
    t = read_text(rel)
    if t is None:
        return None
    try:
        return json.loads(t)
    except Exception as e:
        return {"_parse_error": str(e)}

def read_jsonc(rel):
    t = read_text(rel)
    if t is None:
        return None
    stripped = "\n".join(line for line in t.splitlines() if not line.lstrip().startswith("//"))
    try:
        return json.loads(stripped)
    except Exception as e:
        return {"_parse_error": str(e)}

features = ["check", "diff", "trend", "init"] if feature == "all" else [feature]

check("fixture_repo_clean", git_status == "", f"git status not empty: {git_status!r}")
check("fixture_head_unchanged", actual_head == expected_head, f"expected={expected_head} actual={actual_head}")

if "check" in features:
    check("check.golden_toon.exit", read_exit("check/golden_toon") == 0, str(read_exit("check/golden_toon")))
    toon_out = read_text("check/golden_toon.stdout") or ""
    check("check.golden_toon.shape", "source_sets:" in toon_out and "skipped" in toon_out and "declaration_only" in toon_out, "missing expected TOON keys")

    check("check.golden_json.exit", read_exit("check/golden_json") == 0, str(read_exit("check/golden_json")))
    cj = read_json("check/golden_json.stdout")
    check("check.golden_json.shape", isinstance(cj, dict) and cj.get("files") == 3 and "production" in cj.get("source_sets", {}) and len(cj.get("skipped", [])) == 1 and "src/types.d.ts" in cj.get("declaration_only", []), str(cj))

    check("check.golden_sarif.exit", read_exit("check/golden_sarif") == 0, str(read_exit("check/golden_sarif")))
    cs = read_json("check/golden_sarif.stdout")
    check("check.golden_sarif.shape", isinstance(cs, dict) and cs.get("version") == "2.1.0" and cs.get("runs", [{}])[0].get("tool", {}).get("driver", {}).get("name") == "codeint", "invalid SARIF")

    check("check.golden_md_output.exit", read_exit("check/golden_md_output") == 0, str(read_exit("check/golden_md_output")))
    md_out = read_text("check/report.md") or ""
    check("check.golden_md_output.file", "codeint report" in md_out and "| Source set |" in md_out, "report.md missing expected content")
    check("check.golden_md_output.stdout_empty", (read_text("check/golden_md_output.stdout") or "") == "", "stdout should be empty with --output")

    check("check.golden_all.exit", read_exit("check/golden_all") == 0, str(read_exit("check/golden_all")))

    check("check.over_budget.exit", read_exit("check/over_budget") == 1, str(read_exit("check/over_budget")))
    ob = read_json("check/over_budget.stdout")
    check("check.over_budget.payload", isinstance(ob, dict) and len(ob.get("over_budget", [])) >= 1, str(ob))
    check("check.over_budget.stderr", "Budget" in (read_text("check/over_budget.stderr") or ""), "stderr missing Budget message")

    check("check.invalid_manifest.exit", read_exit("check/invalid_manifest") == 0, str(read_exit("check/invalid_manifest")))
    im = read_json("check/invalid_manifest.stdout")
    im_warnings = im.get("manifest_warnings", []) if isinstance(im, dict) else []
    check("check.invalid_manifest.warning", len(im_warnings) == 1 and "fixtures/go.mod" in im_warnings[0] and "missing module directive" in im_warnings[0], str(im_warnings))

    check("check.invalid_config.exit", read_exit("check/invalid_config") == 2, str(read_exit("check/invalid_config")))
    check("check.missing_path.exit", read_exit("check/missing_path") == 2, str(read_exit("check/missing_path")))
    check("check.non_repo.exit", read_exit("check/non_repo") == 2, str(read_exit("check/non_repo")))

if "diff" in features:
    check("diff.no_regression_1.exit", read_exit("diff/no_regression_1") == 0, str(read_exit("diff/no_regression_1")))
    check("diff.no_regression_2.exit", read_exit("diff/no_regression_2") == 0, str(read_exit("diff/no_regression_2")))
    d1 = read_text("diff/no_regression_1.stdout")
    d2 = read_text("diff/no_regression_2.stdout")
    check("diff.cache_reuse_identical", d1 is not None and d1 == d2, "second cached diff differed")
    check("diff.cache_populated", int(cache_count) >= 1, f"cache_count={cache_count}")

    check("diff.no_regression_md.exit", read_exit("diff/no_regression_md") == 0, str(read_exit("diff/no_regression_md")))
    check("diff.no_regression_md.file", len(read_text("diff/diff.md") or "") > 0, "diff.md empty")

    check("diff.regression_json.exit", read_exit("diff/regression_json") == 1, str(read_exit("diff/regression_json")))
    rj = read_json("diff/regression_json.stdout")
    check("diff.regression_json.payload", isinstance(rj, dict) and len(rj.get("regressions", [])) >= 1, str(rj))
    check("diff.regression_json.stderr", len((read_text("diff/regression_json.stderr") or "").strip()) > 0, "stderr empty on regression")

    check("diff.regression_sarif.exit", read_exit("diff/regression_sarif") == 1, str(read_exit("diff/regression_sarif")))
    rs = read_json("diff/diff.sarif")
    check("diff.regression_sarif.file", isinstance(rs, dict) and rs.get("version") == "2.1.0" and len(rs.get("runs", [{}])[0].get("results", [])) >= 1, "diff.sarif missing new finding")

    check("diff.bad_ref.exit", read_exit("diff/bad_ref") == 2, str(read_exit("diff/bad_ref")))
    check("diff.unborn_head.exit", read_exit("diff/unborn_head") == 2, str(read_exit("diff/unborn_head")))

if "trend" in features:
    check("trend.history_json.exit", read_exit("trend/history_json") == 0, str(read_exit("trend/history_json")))
    tj = read_json("trend/history_json.stdout")
    rows = tj.get("source_sets", {}).get("production", []) if isinstance(tj, dict) else []
    check("trend.history_json.commits", isinstance(tj, dict) and tj.get("commits") == 3 and len(rows) == 3, str(tj))
    check("trend.cache_populated", int(cache_count) >= 1, f"cache_count={cache_count}")

    check("trend.history_toon.exit", read_exit("trend/history_toon") == 0, str(read_exit("trend/history_toon")))
    check("trend.since_json.exit", read_exit("trend/since_json") == 0, str(read_exit("trend/since_json")))
    sj = read_json("trend/since_json.stdout")
    check("trend.since_json.commits", isinstance(sj, dict) and sj.get("commits") == 2, str(sj))

    check("trend.history_md.exit", read_exit("trend/history_md") == 0, str(read_exit("trend/history_md")))
    check("trend.history_md.file", len(read_text("trend/trend.md") or "") > 0, "trend.md empty")

    check("trend.bad_since.exit", read_exit("trend/bad_since") == 2, str(read_exit("trend/bad_since")))
    check("trend.unborn_head.exit", read_exit("trend/unborn_head") == 2, str(read_exit("trend/unborn_head")))

if "init" in features:
    check("init.fresh.exit", read_exit("init/fresh") == 0, str(read_exit("init/fresh")))
    init_cfg = read_jsonc("init/codeint.before.json")
    check("init.fresh.valid_json", isinstance(init_cfg, dict) and "$schema" in init_cfg and "languages" in init_cfg, str(init_cfg))

    check("init.existing.exit", read_exit("init/existing") == 2, str(read_exit("init/existing")))
    before_sha = (read_text("init/codeint.before.sha256") or "").strip()
    after_sha = (read_text("init/codeint.after.sha256") or "").strip()
    check("init.existing.unchanged", before_sha != "" and before_sha == after_sha, f"before={before_sha} after={after_sha}")
    check("init.existing.stderr", "already exists" in (read_text("init/existing.stderr") or ""), "stderr missing already exists")

    check("init.non_repo.exit", read_exit("init/non_repo") == 2, str(read_exit("init/non_repo")))

    check("init.adopt_fresh.exit", read_exit("init/adopt_fresh") == 0, str(read_exit("init/adopt_fresh")))
    adopt_cfg = read_text("init/adopt_fresh.config.json") or ""
    adopt_cfg_json = read_jsonc("init/adopt_fresh.config.json")
    check("init.adopt_fresh.per_file", isinstance(adopt_cfg_json, dict) and adopt_cfg_json.get("regression_limits", {}).get("per_file") is True, adopt_cfg)
    adopt_ratchet = read_json("init/adopt_fresh.ratchet.json")
    adopt_budgets = adopt_cfg_json.get("budgets", {}).get("production", {}) if isinstance(adopt_cfg_json, dict) else {}
    check("init.adopt_fresh.zero_budgets", adopt_budgets.get("erosion") == 0 and "test_gap" not in adopt_budgets, str(adopt_budgets))
    check("init.adopt_fresh.ratchet", isinstance(adopt_ratchet, dict) and "_parse_error" not in adopt_ratchet, str(adopt_ratchet))
    adopt_baseline = read_json("init/adopt_fresh.baseline.json")
    adopt_erosion = []
    if isinstance(adopt_baseline, dict):
        adopt_erosion = adopt_baseline.get("source_sets", {}).get("production", {}).get("erosion", {}).get("findings", [])
    check("init.adopt_fresh.baseline", len(adopt_erosion) >= 1, str(adopt_baseline))
    check("init.adopt_fresh.stdout", "Next steps" in (read_text("init/adopt_fresh.stdout") or ""), "stdout missing Next steps")
    check("init.adopt_check_clean.exit", read_exit("init/adopt_check_clean") == 0, str(read_exit("init/adopt_check_clean")))
    check("init.adopt_check_new.exit", read_exit("init/adopt_check_new") == 1, str(read_exit("init/adopt_check_new")))
    adopt_new_err = read_text("init/adopt_check_new.stderr") or ""
    check("init.adopt_check_new.stderr", "production erosion ratio" in adopt_new_err and "is over its Budget 0" in adopt_new_err and "Ratchet" not in adopt_new_err, adopt_new_err)

    check("init.adopt_rollback.exit", read_exit("init/adopt_rollback") == 2, str(read_exit("init/adopt_rollback")))
    rollback_err = read_text("init/adopt_rollback.stderr") or ""
    check("init.adopt_rollback.stderr", "cannot write Ratchet file" in rollback_err and "could not remove" not in rollback_err, rollback_err)
    rollback_files = (read_text("init/adopt_rollback.files") or "").split()
    check("init.adopt_rollback.no_config", "codeint.jsonc" not in rollback_files and ".codeint" in rollback_files, str(rollback_files))

    check("init.adopt_existing_config.exit", read_exit("init/adopt_existing_config") == 2, str(read_exit("init/adopt_existing_config")))
    ac_before = (read_text("init/adopt_existing_config.before.sha256") or "").strip()
    ac_after = (read_text("init/adopt_existing_config.after.sha256") or "").strip()
    check("init.adopt_existing_config.unchanged", ac_before != "" and ac_before == ac_after, f"before={ac_before} after={ac_after}")
    ac_files = (read_text("init/adopt_existing_config.files") or "").split()
    check("init.adopt_existing_config.no_state", not any(f.startswith(".codeint/") for f in ac_files), str(ac_files))
    ac_err = read_text("init/adopt_existing_config.stderr") or ""
    check("init.adopt_existing_config.stderr", "already exists" in ac_err and "codeint ratchet update" in ac_err and "codeint baseline update" in ac_err, ac_err)

    check("init.adopt_existing_state.exit", read_exit("init/adopt_existing_state") == 2, str(read_exit("init/adopt_existing_state")))
    as_before = (read_text("init/adopt_existing_state.before.sha256") or "").strip()
    as_after = (read_text("init/adopt_existing_state.after.sha256") or "").strip()
    check("init.adopt_existing_state.unchanged", as_before != "" and as_before == as_after, f"before={as_before} after={as_after}")
    as_files = (read_text("init/adopt_existing_state.files") or "").split()
    check("init.adopt_existing_state.no_config", "codeint.jsonc" not in as_files and ".codeint/ratchet.json" not in as_files, str(as_files))
    as_err = read_text("init/adopt_existing_state.stderr") or ""
    check("init.adopt_existing_state.stderr", "already exists" in as_err and "codeint baseline update" in as_err and "codeint baseline update --reset" in as_err, as_err)

verdict = "PASS" if not failures else "FAIL"
summary = {
    "verdict": verdict,
    "build_rev": build_rev,
    "run_id": run_id,
    "feature": feature,
    "checks_passed": len(passed),
    "checks_failed": len(failures),
    "failures": failures,
    "reproduce": f"docs/verification/verify.sh all {feature}",
}

with open(os.path.join(evidence_dir, "verdict.json"), "w", encoding="utf-8") as f:
    json.dump(summary, f, indent=2)
    f.write("\n")

print(f"assert: {verdict} passed={len(passed)} failed={len(failures)} evidence={evidence_dir}/verdict.json")
if failures:
    for item in failures:
        print(f"  FAIL {item['check']}: {item['detail']}", file=sys.stderr)
    sys.exit(1)
PY
}

cmd_cleanup() {
  local run_dir="${1:?usage: verify.sh cleanup <run-dir>}"
  if [ -d "$run_dir" ]; then
    rm -rf "$run_dir"
    printf 'cleanup: removed %s\n' "$run_dir"
  else
    printf 'cleanup: already absent %s\n' "$run_dir"
  fi
}

cmd_all() {
  local feature="${1:-all}"
  export CODEINT_VERIFY_OWNER="${CODEINT_VERIFY_OWNER:-$(id -un):$$}"
  local launch_out
  launch_out="$(cmd_launch)"
  local run_dir
  run_dir="$(printf '%s\n' "$launch_out" | awk -F= '$1=="RUN_DIR"{print $2}')"
  local evidence_dir
  evidence_dir="$(printf '%s\n' "$launch_out" | awk -F= '$1=="EVIDENCE_DIR"{print $2}')"
  local status=0
  cmd_doctor "$run_dir" || status=$?
  if [ "$status" -eq 0 ]; then
    cmd_drive "$run_dir" "$feature" || status=$?
  fi
  if [ "$status" -eq 0 ]; then
    cmd_assert "$run_dir" "$feature" || status=$?
  fi
  cmd_cleanup "$run_dir"
  printf 'evidence_dir=%s\n' "$evidence_dir"
  return "$status"
}

subcommand="${1:-}"
if [ -z "$subcommand" ]; then
  printf 'usage: %s <launch|doctor|drive|assert|cleanup|all> [args...]\n' "$0" >&2
  exit 2
fi
shift

case "$subcommand" in
  launch) cmd_launch "$@" ;;
  doctor) cmd_doctor "$@" ;;
  drive) cmd_drive "$@" ;;
  assert) cmd_assert "$@" ;;
  cleanup) cmd_cleanup "$@" ;;
  all) cmd_all "$@" ;;
  *)
    printf 'unknown subcommand: %s\n' "$subcommand" >&2
    exit 2
    ;;
esac
