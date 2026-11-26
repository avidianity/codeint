#!/usr/bin/env bash
set -euo pipefail

ALL_FEATURES=(check diff trend init ratchet baseline plugin-test impact trace guard churn coupling report config schema doctor agent mcp update)

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

fresh_work_copy() {
  local src="$1"
  local dst="$2"
  rm -rf "$dst"
  cp -R "$src" "$dst"
}

ensure_wasm_fixtures() {
  local target_dir="$REPO_ROOT/target/wasm-fixtures"
  local out_dir="$target_dir/wasm32-wasip2/debug"
  local mini="$out_dir/sample_mini_plugin.wasm"
  local metric="$out_dir/sample_long_functions_plugin.wasm"
  local bad="$out_dir/out_of_range_ratio_metric_plugin.wasm"

  if [ ! -f "$mini" ]; then
    cargo build --manifest-path "$REPO_ROOT/crates/plugin-host/tests/fixtures/sample-plugin/Cargo.toml" \
      --target wasm32-wasip2 --locked --offline --target-dir "$target_dir" >&2
  fi
  if [ ! -f "$metric" ]; then
    cargo build --manifest-path "$REPO_ROOT/crates/plugin-host/tests/fixtures/sample-metric-plugin/Cargo.toml" \
      --target wasm32-wasip2 --locked --offline --target-dir "$target_dir" >&2
  fi
  if [ ! -f "$bad" ]; then
    cargo build --manifest-path "$REPO_ROOT/crates/plugin-host/tests/fixtures/sandbox-probes/Cargo.toml" \
      -p out-of-range-ratio-metric-plugin \
      --target wasm32-wasip2 --locked --offline --target-dir "$target_dir" >&2
  fi
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
  local bin_version
  bin_version="$("$bin_path" --version | awk '{print $2}')"
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
BIN_VERSION='${bin_version}'
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

run_case_stdin() {
  local workdir="$1"
  local out_prefix="$2"
  local stdin_file="$3"
  shift 3
  local exit_code=0
  printf '%s < %s\n' "$*" "$stdin_file" > "${out_prefix}.cmd"
  (
    cd "$workdir"
    CODEINT_CACHE_DIR="$CACHE_DIR" "$@" < "$stdin_file"
  ) > "${out_prefix}.stdout" 2> "${out_prefix}.stderr" || exit_code=$?
  printf '%s\n' "$exit_code" > "${out_prefix}.exit"
}

run_case_env() {
  local workdir="$1"
  local out_prefix="$2"
  local env_pair="$3"
  shift 3
  local exit_code=0
  printf '%s %s\n' "$env_pair" "$*" > "${out_prefix}.cmd"
  (
    cd "$workdir"
    env CODEINT_CACHE_DIR="$CACHE_DIR" "$env_pair" "$@"
  ) > "${out_prefix}.stdout" 2> "${out_prefix}.stderr" || exit_code=$?
  printf '%s\n' "$exit_code" > "${out_prefix}.exit"
}

drive_check() {
  local dir="$EVIDENCE_DIR/check"
  mkdir -p "$dir"

  run_case "$FIXTURE_REPO" "$dir/golden_toon" "$BIN_PATH" check
  run_case "$FIXTURE_REPO" "$dir/golden_text" "$BIN_PATH" check --format text
  run_case "$FIXTURE_REPO" "$dir/golden_json" "$BIN_PATH" check --format json
  run_case "$FIXTURE_REPO" "$dir/golden_sarif" "$BIN_PATH" check --format sarif
  run_case "$FIXTURE_REPO" "$dir/golden_md_output" "$BIN_PATH" check --format markdown --output "$dir/report.md"
  run_case "$FIXTURE_REPO" "$dir/golden_all" "$BIN_PATH" check --all --format json

  local work_fmt="$RUN_DIR/work-check-format"
  rm -rf "$work_fmt"
  cp -R "$FIXTURE_REPO" "$work_fmt"
  printf '{"format":"text"}\n' > "$work_fmt/codeint.json"
  run_case "$work_fmt" "$dir/config_format_text" "$BIN_PATH" check

  local work_budget="$RUN_DIR/work-check-budget"
  fresh_work_copy "$FIXTURE_REPO" "$work_budget"
  printf '{"budgets":{"production":{"verbosity":0}}}\n' > "$work_budget/codeint.json"
  run_case "$work_budget" "$dir/over_budget" "$BIN_PATH" check --format json

  local work_badcfg="$RUN_DIR/work-check-badcfg"
  fresh_work_copy "$FIXTURE_REPO" "$work_badcfg"
  printf '{"budget":{}}\n' > "$work_badcfg/codeint.json"
  run_case "$work_badcfg" "$dir/invalid_config" "$BIN_PATH" check

  local work_manifest="$RUN_DIR/work-check-manifest"
  fresh_work_copy "$FIXTURE_REPO" "$work_manifest"
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
  run_case "$FIXTURE_REPO" "$dir/no_regression_text" "$BIN_PATH" diff main --head feature --format text

  local work_reg="$RUN_DIR/work-diff-regression"
  fresh_work_copy "$FIXTURE_REPO" "$work_reg"
  write_eroded_ts "$work_reg/src/route.ts" "route" 16
  run_case "$work_reg" "$dir/regression_json" "$BIN_PATH" diff main --format json
  run_case "$work_reg" "$dir/regression_sarif" "$BIN_PATH" diff main --format sarif --output "$dir/diff.sarif"
  printf '{"format":"text"}\n' > "$work_reg/codeint.json"
  run_case "$work_reg" "$dir/regression_text" "$BIN_PATH" diff main

  local work_pf="$RUN_DIR/work-diff-per-file"
  fresh_work_copy "$FIXTURE_REPO" "$work_pf"
  printf '{"regression_limits":{"per_file":true}}\n' > "$work_pf/codeint.json"
  run_case "$work_pf" "$dir/per_file_clean" "$BIN_PATH" diff main --head feature --format json
  git -C "$work_pf" checkout -q main
  printf '{"regression_limits":{"per_file":true}}\n' > "$work_pf/codeint.json"
  printf 'export function parse(code: number): number {\n  return code + 1;\n}\n' > "$work_pf/src/parse.ts"
  write_eroded_ts "$work_pf/src/sign.ts" "sign" 11
  run_case "$work_pf" "$dir/per_file_regression" "$BIN_PATH" diff main --format json

  local work_nfb="$RUN_DIR/work-diff-new-file-budget"
  fresh_work_copy "$FIXTURE_REPO" "$work_nfb"
  printf '{"new_file_budgets":{"production":{"erosion":0}},"regression_limits":{"erosion":{"ratio_rise":1,"new_findings":"never"}}}\n' > "$work_nfb/codeint.json"
  write_eroded_ts "$work_nfb/src/added.ts" "added" 14
  run_case "$work_nfb" "$dir/new_file_budget_fail" "$BIN_PATH" diff main --format json
  run_case "$work_nfb" "$dir/new_file_budget_sarif" "$BIN_PATH" diff main --format sarif --output "$dir/new_file_budget.sarif"

  local work_nfb_clean="$RUN_DIR/work-diff-new-file-budget-clean"
  fresh_work_copy "$FIXTURE_REPO" "$work_nfb_clean"
  git -C "$work_nfb_clean" checkout -q main
  printf '{"new_file_budgets":{"production":{"erosion":0}}}\n' > "$work_nfb_clean/codeint.json"
  printf '\nexport function abs(n: number): number {\n  return n < 0 ? -n : n;\n}\n' >> "$work_nfb_clean/src/sign.ts"
  run_case "$work_nfb_clean" "$dir/new_file_budget_clean" "$BIN_PATH" diff main --format json

  local work_nfb_bad="$RUN_DIR/work-diff-new-file-budget-invalid"
  fresh_work_copy "$FIXTURE_REPO" "$work_nfb_bad"
  printf '{"new_file_budgets":{"test":{"orphan_files":0}}}\n' > "$work_nfb_bad/codeint.json"
  run_case "$work_nfb_bad" "$dir/new_file_budget_invalid" "$BIN_PATH" diff main

  run_case "$FIXTURE_REPO" "$dir/bad_ref" "$BIN_PATH" diff no-such-branch
  run_case "$EMPTY_REPO" "$dir/unborn_head" "$BIN_PATH" diff main
}

drive_trend() {
  local dir="$EVIDENCE_DIR/trend"
  mkdir -p "$dir"

  run_case "$FIXTURE_REPO" "$dir/history_json" "$BIN_PATH" trend --format json
  run_case "$FIXTURE_REPO" "$dir/history_toon" "$BIN_PATH" trend --format toon
  run_case "$FIXTURE_REPO" "$dir/history_text" "$BIN_PATH" trend --format text
  run_case "$FIXTURE_REPO" "$dir/since_json" "$BIN_PATH" trend --since 2026-11-05 --format json
  run_case "$FIXTURE_REPO" "$dir/history_md" "$BIN_PATH" trend --format markdown --output "$dir/trend.md"

  local work_trend_badfmt="$RUN_DIR/work-trend-badfmt"
  rm -rf "$work_trend_badfmt"
  cp -R "$FIXTURE_REPO" "$work_trend_badfmt"
  printf '{"format":"sarif"}\n' > "$work_trend_badfmt/codeint.json"
  run_case "$work_trend_badfmt" "$dir/config_unsupported_format" "$BIN_PATH" trend

  run_case "$FIXTURE_REPO" "$dir/bad_since" "$BIN_PATH" trend --since 2026-02-30
  run_case "$EMPTY_REPO" "$dir/unborn_head" "$BIN_PATH" trend
}

drive_init() {
  local dir="$EVIDENCE_DIR/init"
  mkdir -p "$dir"

  local work_init="$RUN_DIR/work-init"
  fresh_work_copy "$INIT_REPO" "$work_init"

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
  fresh_work_copy "$INIT_REPO" "$work_adopt"
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
  run_case "$work_adopt" "$dir/adopt_diff_clean" "$BIN_PATH" diff HEAD --format json
  write_eroded_ts "$work_adopt/src/fresh.ts" "fresh" 16
  printf 'export const legacy = 1;\n' > "$work_adopt/src/legacy.ts"
  run_case "$work_adopt" "$dir/adopt_check_new" "$BIN_PATH" check --format json
  run_case "$work_adopt" "$dir/adopt_diff_new" "$BIN_PATH" diff HEAD --format json

  local work_adopt_rollback="$RUN_DIR/work-init-adopt-rollback"
  fresh_work_copy "$INIT_REPO" "$work_adopt_rollback"
  write_eroded_ts "$work_adopt_rollback/src/legacy.ts" "legacy" 16
  printf 'not a folder\n' > "$work_adopt_rollback/.codeint"
  run_case "$work_adopt_rollback" "$dir/adopt_rollback" "$BIN_PATH" init --adopt
  find "$work_adopt_rollback" -path "$work_adopt_rollback/.git" -prune -o -type f -print | sed "s|^$work_adopt_rollback/||" | sort > "$dir/adopt_rollback.files"

  local work_adopt_cfg="$RUN_DIR/work-init-adopt-config"
  fresh_work_copy "$INIT_REPO" "$work_adopt_cfg"
  printf '{"budgets":{}}\n' > "$work_adopt_cfg/codeint.jsonc"
  sha256_file "$work_adopt_cfg/codeint.jsonc" > "$dir/adopt_existing_config.before.sha256"
  run_case "$work_adopt_cfg" "$dir/adopt_existing_config" "$BIN_PATH" init --adopt
  sha256_file "$work_adopt_cfg/codeint.jsonc" > "$dir/adopt_existing_config.after.sha256"
  find "$work_adopt_cfg" -path "$work_adopt_cfg/.git" -prune -o -type f -print | sed "s|^$work_adopt_cfg/||" | sort > "$dir/adopt_existing_config.files"

  local work_adopt_state="$RUN_DIR/work-init-adopt-state"
  fresh_work_copy "$INIT_REPO" "$work_adopt_state"
  mkdir -p "$work_adopt_state/.codeint"
  printf '{"version":1,"findings":[]}\n' > "$work_adopt_state/.codeint/baseline.json"
  sha256_file "$work_adopt_state/.codeint/baseline.json" > "$dir/adopt_existing_state.before.sha256"
  run_case "$work_adopt_state" "$dir/adopt_existing_state" "$BIN_PATH" init --adopt
  sha256_file "$work_adopt_state/.codeint/baseline.json" > "$dir/adopt_existing_state.after.sha256"
  find "$work_adopt_state" -path "$work_adopt_state/.git" -prune -o -type f -print | sed "s|^$work_adopt_state/||" | sort > "$dir/adopt_existing_state.files"
}

drive_ratchet() {
  local dir="$EVIDENCE_DIR/ratchet"
  mkdir -p "$dir"

  local work="$RUN_DIR/work-ratchet"
  fresh_work_copy "$INIT_REPO" "$work"
  write_eroded_ts "$work/src/eroded.ts" "eroded" 11
  printf 'export function cleanA(n: number): number { return n + 1; }\nexport function cleanB(n: number): number { return n + 2; }\nexport function cleanC(n: number): number { return n + 3; }\n' > "$work/src/clean.ts"
  git -C "$work" add .
  git_commit_deterministic "$work" "2026-11-17T12:00:00Z" "Add eroded and clean"

  run_case "$work" "$dir/create_json" "$BIN_PATH" ratchet update --format json
  if [ -f "$work/.codeint/ratchet.json" ]; then
    cp "$work/.codeint/ratchet.json" "$dir/ratchet.created.json"
    sha256_file "$work/.codeint/ratchet.json" > "$dir/ratchet.before_loosen.sha256"
  fi
  run_case "$work" "$dir/check_pass" "$BIN_PATH" check --format json

  write_eroded_ts "$work/src/more.ts" "more" 16
  run_case "$work" "$dir/check_fail" "$BIN_PATH" check --format json
  run_case "$work" "$dir/diff_fail" "$BIN_PATH" diff main --format json
  run_case "$work" "$dir/loosen_refused" "$BIN_PATH" ratchet update
  if [ -f "$work/.codeint/ratchet.json" ]; then
    sha256_file "$work/.codeint/ratchet.json" > "$dir/ratchet.after_loosen_refused.sha256"
  fi
  run_case "$work" "$dir/loosen_reset" "$BIN_PATH" ratchet update --reset --format toon
  if [ -f "$work/.codeint/ratchet.json" ]; then
    cp "$work/.codeint/ratchet.json" "$dir/ratchet.reset.json"
  fi

  printf 'export function eroded(code: number): number { return code + 1; }\n' > "$work/src/eroded.ts"
  printf 'export function more(code: number): number { return code + 2; }\n' > "$work/src/more.ts"
  run_case "$work" "$dir/tighten" "$BIN_PATH" ratchet update --format json
  if [ -f "$work/.codeint/ratchet.json" ]; then
    cp "$work/.codeint/ratchet.json" "$dir/ratchet.tightened.json"
  fi

  python3 - "$work/.codeint/ratchet.json" <<'PY'
import json, sys
p = sys.argv[1]
with open(p, "r", encoding="utf-8") as f:
    data = json.load(f)
data["source_sets"]["production"]["erosion"]["metric_version"] = 999
with open(p, "w", encoding="utf-8") as f:
    json.dump(data, f, indent=2)
PY
  run_case "$work" "$dir/version_mismatch" "$BIN_PATH" check

  printf '{not-json\n' > "$work/.codeint/ratchet.json"
  run_case "$work" "$dir/corrupt_refused" "$BIN_PATH" ratchet update
  run_case "$work" "$dir/corrupt_reset" "$BIN_PATH" ratchet update --reset --format json
  if [ -f "$work/.codeint/ratchet.json" ]; then
    cp "$work/.codeint/ratchet.json" "$dir/ratchet.recovered.json"
  fi

  local work_ws="$RUN_DIR/work-ratchet-workspace"
  rm -rf "$work_ws"
  mkdir -p "$work_ws/packages/a/src"
  git -C "$work_ws" init -b main -q
  printf '{"name":"root","private":true,"workspaces":["packages/*"]}\n' > "$work_ws/package.json"
  printf '{"name":"pkg-a"}\n' > "$work_ws/packages/a/package.json"
  printf 'export function ok(x: number): number { return x + 1; }\n' > "$work_ws/packages/a/src/a.ts"
  git -C "$work_ws" add .
  git_commit_deterministic "$work_ws" "2026-11-17T12:00:00Z" "Workspace init"
  run_case "$work_ws" "$dir/package_ratchet_create" "$BIN_PATH" ratchet update
  run_case "$work_ws" "$dir/package_skipped" "$BIN_PATH" check --package packages/a --format json
}

drive_baseline() {
  local dir="$EVIDENCE_DIR/baseline"
  mkdir -p "$dir"

  local work="$RUN_DIR/work-baseline"
  fresh_work_copy "$INIT_REPO" "$work"
  write_eroded_ts "$work/src/legacy.ts" "legacy" 14
  git -C "$work" add .
  git_commit_deterministic "$work" "2026-11-17T12:00:00Z" "Add legacy eroded"

  run_case "$work" "$dir/create_json" "$BIN_PATH" baseline update --format json
  if [ -f "$work/.codeint/baseline.json" ]; then
    cp "$work/.codeint/baseline.json" "$dir/baseline.created.json"
  fi

  printf '{"budgets":{"production":{"erosion":0}}}\n' > "$work/codeint.json"
  run_case "$work" "$dir/check_hides" "$BIN_PATH" check --format json
  run_case "$work" "$dir/check_ignore_baseline" "$BIN_PATH" check --ignore-baseline --format json

  printf 'export function legacy(code: number): number { return code + 1; }\n' > "$work/src/legacy.ts"
  write_eroded_ts "$work/src/fresh.ts" "fresh" 14
  run_case "$work" "$dir/prune_and_skip_new" "$BIN_PATH" baseline update --format json
  if [ -f "$work/.codeint/baseline.json" ]; then
    cp "$work/.codeint/baseline.json" "$dir/baseline.pruned.json"
  fi

  run_case "$work" "$dir/reset_adds_new" "$BIN_PATH" baseline update --reset --format toon
  if [ -f "$work/.codeint/baseline.json" ]; then
    cp "$work/.codeint/baseline.json" "$dir/baseline.reset.json"
  fi
  run_case "$work" "$dir/reset_check_pass" "$BIN_PATH" check --format json

  python3 - "$work/.codeint/baseline.json" <<'PY'
import json, sys
p = sys.argv[1]
with open(p, "r", encoding="utf-8") as f:
    data = json.load(f)
data["source_sets"]["production"]["erosion"]["metric_version"] = 999
with open(p, "w", encoding="utf-8") as f:
    json.dump(data, f, indent=2)
PY
  run_case "$work" "$dir/version_mismatch" "$BIN_PATH" check

  printf '{not-json\n' > "$work/.codeint/baseline.json"
  run_case "$work" "$dir/corrupt_refused" "$BIN_PATH" baseline update
  run_case "$work" "$dir/corrupt_reset" "$BIN_PATH" baseline update --reset --format json
  if [ -f "$work/.codeint/baseline.json" ]; then
    cp "$work/.codeint/baseline.json" "$dir/baseline.recovered.json"
  fi
}

drive_plugin_test() {
  local dir="$EVIDENCE_DIR/plugin-test"
  mkdir -p "$dir"

  ensure_wasm_fixtures
  local wasm_dir="$REPO_ROOT/target/wasm-fixtures/wasm32-wasip2/debug"
  local sample_source="$RUN_DIR/sample.mini"
  printf 'fn demo() {\n  return 1;\n}\n' > "$sample_source"
  local corrupt_wasm="$RUN_DIR/corrupt.wasm"
  printf 'not a valid wasm component\n' > "$corrupt_wasm"

  run_case "$RUN_DIR" "$dir/language_json" "$BIN_PATH" plugin test "$wasm_dir/sample_mini_plugin.wasm" --format json
  run_case "$RUN_DIR" "$dir/language_source_text" "$BIN_PATH" plugin test "$wasm_dir/sample_mini_plugin.wasm" --kind language --source "$sample_source" --format text
  run_case "$RUN_DIR" "$dir/metric_toon" "$BIN_PATH" plugin test "$wasm_dir/sample_long_functions_plugin.wasm" --kind metric --format toon
  run_case "$RUN_DIR" "$dir/contract_fail_json" "$BIN_PATH" plugin test "$wasm_dir/out_of_range_ratio_metric_plugin.wasm" --kind metric --format json
  run_case "$RUN_DIR" "$dir/corrupt_wasm" "$BIN_PATH" plugin test "$corrupt_wasm" --format json
  run_case "$RUN_DIR" "$dir/missing_file" "$BIN_PATH" plugin test "$RUN_DIR/missing.wasm"
}

drive_impact() {
  local dir="$EVIDENCE_DIR/impact"
  mkdir -p "$dir"

  local work="$RUN_DIR/work-impact"
  rm -rf "$work"
  mkdir -p "$work/src"
  git -C "$work" init -b main -q
  printf 'export function a(x: number): number { return x + 1; }\n' > "$work/src/a.ts"
  printf 'import { a } from "./a";\nexport function b(x: number): number { return a(x) + 1; }\n' > "$work/src/b.ts"
  printf 'import { b } from "./b";\nexport function c(x: number): number { return b(x) + 1; }\n' > "$work/src/c.ts"
  printf 'import { a } from "./a";\nexport function testA(): number { return a(1); }\n' > "$work/src/a.test.ts"
  printf 'export function isolated(x: number): number { return x; }\n' > "$work/src/isolated.ts"
  git -C "$work" add .
  git_commit_deterministic "$work" "2026-11-16T12:00:00Z" "Base graph"

  git -C "$work" checkout -b feature -q
  printf 'export function a(x: number): number { return x + 2; }\n' > "$work/src/a.ts"
  git -C "$work" add .
  git_commit_deterministic "$work" "2026-11-17T12:00:00Z" "Modify a"

  run_case "$work" "$dir/direct_and_transitive_json" "$BIN_PATH" impact src/a.ts --format json
  run_case "$work" "$dir/depth_one_text" "$BIN_PATH" impact src/a.ts --depth 1 --format text
  run_case "$work" "$dir/changed_toon" "$BIN_PATH" impact --changed main --format toon
  run_case "$work" "$dir/no_dependents_output" "$BIN_PATH" impact src/isolated.ts --format json --output "$dir/impact.json"
  run_case "$work" "$dir/missing_package" "$BIN_PATH" impact src/a.ts --package packages/missing
  run_case "$work" "$dir/missing_file" "$BIN_PATH" impact src/no-such-file.ts
  run_case "$NON_REPO" "$dir/non_repo" "$BIN_PATH" impact index.ts
}

drive_trace() {
  local dir="$EVIDENCE_DIR/trace"
  mkdir -p "$dir"

  local work="$RUN_DIR/work-trace"
  rm -rf "$work"
  mkdir -p "$work/src"
  git -C "$work" init -b main -q
  printf 'export function leaf(x: number): number { return x + 1; }\n' > "$work/src/leaf.ts"
  printf 'import { leaf } from "./leaf";\nexport function left(x: number): number { return leaf(x); }\n' > "$work/src/left.ts"
  printf 'import { leaf } from "./leaf";\nexport function right(x: number): number { return leaf(x); }\n' > "$work/src/right.ts"
  printf 'import { left } from "./left";\nimport { right } from "./right";\nexport function entry(x: number): number { return left(x) + right(x); }\n' > "$work/src/entry.ts"
  printf 'export function isolated(x: number): number { return x; }\n' > "$work/src/isolated.ts"
  git -C "$work" add .
  git_commit_deterministic "$work" "2026-11-16T12:00:00Z" "Diamond graph"

  run_case "$work" "$dir/connected_json" "$BIN_PATH" trace src/entry.ts src/leaf.ts --format json
  run_case "$work" "$dir/all_paths_json" "$BIN_PATH" trace src/entry.ts src/leaf.ts --all --max-paths 5 --format json
  run_case "$work" "$dir/disconnected_toon" "$BIN_PATH" trace src/entry.ts src/isolated.ts --format toon
  run_case "$work" "$dir/text_output" "$BIN_PATH" trace src/entry.ts src/leaf.ts --format text --output "$dir/trace.txt"
  run_case "$work" "$dir/missing_package" "$BIN_PATH" trace src/entry.ts src/leaf.ts --package packages/missing
  run_case "$work" "$dir/unknown_file" "$BIN_PATH" trace src/missing.ts src/leaf.ts
  run_case "$NON_REPO" "$dir/non_repo" "$BIN_PATH" trace index.ts index.ts
}

drive_guard() {
  local dir="$EVIDENCE_DIR/guard"
  mkdir -p "$dir"

  run_case "$FIXTURE_REPO" "$dir/unconfigured_json" "$BIN_PATH" guard src/sign.ts --format json

  local work="$RUN_DIR/work-guard"
  rm -rf "$work"
  mkdir -p "$work/src/domain" "$work/src/ui"
  git -C "$work" init -b main -q
  cat > "$work/codeint.json" <<'EOF'
{
  "boundaries": {
    "zones": {
      "domain": { "patterns": ["src/domain/**"] },
      "ui": { "patterns": ["src/ui/**"] }
    },
    "rules": [
      { "from": "domain", "deny": ["ui"], "deny_external": ["axios"] },
      { "from": "ui", "allow": ["domain"] }
    ]
  }
}
EOF
  printf 'export const order = 1;\n' > "$work/src/domain/order.ts"
  git -C "$work" add .
  git_commit_deterministic "$work" "2026-11-16T12:00:00Z" "Add boundaries"

  run_case "$work" "$dir/configured_existing_json" "$BIN_PATH" guard src/domain/order.ts --format json
  run_case "$work" "$dir/configured_planned_toon" "$BIN_PATH" guard src/ui/planned.ts --format toon
  run_case "$work" "$dir/configured_text" "$BIN_PATH" guard src/domain/order.ts --format text
  run_case "$work" "$dir/outside_repo" "$BIN_PATH" guard ../outside.ts
  run_case "$NON_REPO" "$dir/non_repo" "$BIN_PATH" guard index.ts
}

drive_churn() {
  local dir="$EVIDENCE_DIR/churn"
  mkdir -p "$dir"

  local work="$RUN_DIR/work-churn"
  rm -rf "$work"
  mkdir -p "$work/src"
  git -C "$work" init -b main -q
  write_eroded_ts "$work/src/hot.ts" "hot" 12
  printf 'export function calm(x: number): number { return x; }\n' > "$work/src/calm.ts"
  git -C "$work" add .
  git_commit_deterministic "$work" "2026-11-02T12:00:00Z" "Add hot v1"

  write_eroded_ts "$work/src/hot.ts" "hot" 13
  git -C "$work" add .
  git_commit_deterministic "$work" "2026-11-09T12:00:00Z" "Touch hot v2"

  write_eroded_ts "$work/src/hot.ts" "hot" 14
  git -C "$work" add .
  git_commit_deterministic "$work" "2026-11-16T12:00:00Z" "Touch hot v3"

  run_case "$work" "$dir/commits_json" "$BIN_PATH" churn --commits 10 --format json
  run_case "$work" "$dir/since_toon" "$BIN_PATH" churn --since 2026-11-05 --format toon
  run_case "$work" "$dir/text_all" "$BIN_PATH" churn --commits 10 --all --format text
  run_case "$work" "$dir/markdown_output" "$BIN_PATH" churn --commits 10 --format markdown --output "$dir/churn.md"
  run_case "$work" "$dir/bad_since" "$BIN_PATH" churn --since 2026-02-30
  run_case "$EMPTY_REPO" "$dir/unborn_head" "$BIN_PATH" churn
}

drive_coupling() {
  local dir="$EVIDENCE_DIR/coupling"
  mkdir -p "$dir"

  local work="$RUN_DIR/work-coupling"
  rm -rf "$work"
  mkdir -p "$work/src"
  git -C "$work" init -b main -q
  printf 'export const alpha = 1;\n' > "$work/src/alpha.ts"
  printf 'export const beta = 1;\n' > "$work/src/beta.ts"
  printf 'export const gamma = 1;\n' > "$work/src/gamma.ts"
  printf 'export const delta = 1;\n' > "$work/src/delta.ts"
  printf 'export const alphaTest = 1;\n' > "$work/src/alpha.test.ts"
  git -C "$work" add .
  git_commit_deterministic "$work" "2026-11-01T12:00:00Z" "Bulk commit 1"

  printf 'export const alpha = 2;\n' > "$work/src/alpha.ts"
  printf 'export const beta = 2;\n' > "$work/src/beta.ts"
  printf 'export const alphaTest = 2;\n' > "$work/src/alpha.test.ts"
  git -C "$work" add .
  git_commit_deterministic "$work" "2026-11-05T12:00:00Z" "Co-change 2"

  printf 'export const alpha = 3;\n' > "$work/src/alpha.ts"
  printf 'export const beta = 3;\n' > "$work/src/beta.ts"
  printf 'export const alphaTest = 3;\n' > "$work/src/alpha.test.ts"
  git -C "$work" add .
  git_commit_deterministic "$work" "2026-11-10T12:00:00Z" "Co-change 3"

  printf 'export const alpha = 4;\n' > "$work/src/alpha.ts"
  printf 'export const beta = 4;\n' > "$work/src/beta.ts"
  printf 'export const alphaTest = 4;\n' > "$work/src/alpha.test.ts"
  git -C "$work" add .
  git_commit_deterministic "$work" "2026-11-15T12:00:00Z" "Co-change 4"

  run_case "$work" "$dir/pairs_json" "$BIN_PATH" coupling --commits 10 --min-co-changes 3 --min-ratio 0.5 --format json
  run_case "$work" "$dir/bulk_excluded_json" "$BIN_PATH" coupling --commits 10 --max-commit-files 3 --min-co-changes 2 --min-ratio 0.5 --format json
  run_case "$work" "$dir/include_tests_toon" "$BIN_PATH" coupling --commits 10 --include-tests --min-co-changes 2 --min-ratio 0.5 --format toon
  run_case "$work" "$dir/markdown_output" "$BIN_PATH" coupling --commits 10 --min-co-changes 2 --format markdown --output "$dir/coupling.md"
  run_case "$work" "$dir/text_format" "$BIN_PATH" coupling --commits 10 --min-co-changes 2 --format text
  run_case "$work" "$dir/invalid_ratio" "$BIN_PATH" coupling --min-ratio 1.5
  run_case "$EMPTY_REPO" "$dir/unborn_head" "$BIN_PATH" coupling
}

drive_report() {
  local dir="$EVIDENCE_DIR/report"
  mkdir -p "$dir"

  local work="$RUN_DIR/work-report"
  rm -rf "$work"
  mkdir -p "$work"

  run_case "$FIXTURE_REPO" "$dir/seed_check" "$BIN_PATH" check --format json --output "$work/check.json"
  run_case "$FIXTURE_REPO" "$dir/seed_diff" "$BIN_PATH" diff main --head feature --format json --output "$work/diff.json"
  printf '{not-json\n' > "$work/malformed.json"
  printf '{"schema_version":999}\n' > "$work/unsupported.json"

  run_case "$work" "$dir/check_to_sarif_output" "$BIN_PATH" report "$work/check.json" --format sarif --output "$dir/report.sarif"
  run_case "$work" "$dir/check_to_markdown" "$BIN_PATH" report "$work/check.json" --format markdown
  run_case_stdin "$work" "$dir/check_to_text_stdin" "$work/check.json" "$BIN_PATH" report - --format text
  run_case "$work" "$dir/check_to_text_all" "$BIN_PATH" report "$work/check.json" --format text --all
  run_case "$work" "$dir/diff_to_toon" "$BIN_PATH" report "$work/diff.json" --format toon
  run_case "$work" "$dir/diff_to_json" "$BIN_PATH" report "$work/diff.json" --format json
  run_case "$work" "$dir/missing_input" "$BIN_PATH" report "$work/no-such.json" --format sarif
  run_case "$work" "$dir/malformed_json" "$BIN_PATH" report "$work/malformed.json" --format toon
  run_case "$work" "$dir/unsupported_schema" "$BIN_PATH" report "$work/unsupported.json" --format toon
}

drive_config() {
  local dir="$EVIDENCE_DIR/config"
  mkdir -p "$dir"

  run_case "$INIT_REPO" "$dir/resolved_default_json" "$BIN_PATH" config --resolved --format json

  local work="$RUN_DIR/work-config"
  fresh_work_copy "$INIT_REPO" "$work"
  printf '{"regression_limits":{"per_file":true}}\n' > "$work/codeint.json"
  run_case "$work" "$dir/resolved_file_and_cli_json" "$BIN_PATH" config --resolved --format json \
    --budget production.erosion=0.25 \
    --source-set test=spec/** \
    --plugin-memory-mb 128 \
    --plugin-timeout-ms 10000
  run_case "$work" "$dir/resolved_subfolder_text" "$BIN_PATH" config --resolved src --format text
  run_case "$work" "$dir/resolved_toon" "$BIN_PATH" config --resolved --format toon
  run_case "$work" "$dir/invalid_override" "$BIN_PATH" config --resolved --budget production.erosion=2.0

  local work_bad="$RUN_DIR/work-config-bad"
  fresh_work_copy "$INIT_REPO" "$work_bad"
  printf '{"unknown_option":true}\n' > "$work_bad/codeint.json"
  run_case "$work_bad" "$dir/invalid_config" "$BIN_PATH" config --resolved

  run_case "$NON_REPO" "$dir/non_repo" "$BIN_PATH" config --resolved
}

drive_schema() {
  local dir="$EVIDENCE_DIR/schema"
  mkdir -p "$dir"

  run_case "$FIXTURE_REPO" "$dir/json_default" "$BIN_PATH" schema
  run_case "$FIXTURE_REPO" "$dir/toon_format" "$BIN_PATH" schema --format toon
  run_case "$FIXTURE_REPO" "$dir/output_file" "$BIN_PATH" schema --format json --output "$dir/schema.json"
  run_case "$NON_REPO" "$dir/non_repo" "$BIN_PATH" schema --format json
  run_case "$FIXTURE_REPO" "$dir/unknown_option" "$BIN_PATH" schema --no-such-flag
}

drive_doctor() {
  local dir="$EVIDENCE_DIR/doctor"
  mkdir -p "$dir"

  local work_ok="$RUN_DIR/work-doctor-ok"
  fresh_work_copy "$INIT_REPO" "$work_ok"
  printf '{}\n' > "$work_ok/codeint.json"
  run_case "$work_ok" "$dir/healthy_with_config_json" "$BIN_PATH" doctor --format json

  run_case "$FIXTURE_REPO" "$dir/no_config_warn_text" "$BIN_PATH" doctor --format text
  run_case "$FIXTURE_REPO" "$dir/subfolder_toon" "$BIN_PATH" doctor src --format toon

  local work_bad="$RUN_DIR/work-doctor-badcfg"
  fresh_work_copy "$INIT_REPO" "$work_bad"
  printf '{"budget":{}}\n' > "$work_bad/codeint.json"
  run_case "$work_bad" "$dir/invalid_config_fail" "$BIN_PATH" doctor --format json

  local work_plugin="$RUN_DIR/work-doctor-missing-plugin"
  fresh_work_copy "$INIT_REPO" "$work_plugin"
  printf '{"plugins":[{"name":"missing","kind":"language","path":"plugins/missing.wasm","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}]}\n' > "$work_plugin/codeint.json"
  run_case "$work_plugin" "$dir/missing_plugin_fail" "$BIN_PATH" doctor --format json

  run_case "$NON_REPO" "$dir/non_repo_warn" "$BIN_PATH" doctor --format json
}

drive_agent() {
  local dir="$EVIDENCE_DIR/agent"
  mkdir -p "$dir"

  local agent_home="$RUN_DIR/work-agent-home"
  rm -rf "$agent_home"
  mkdir -p "$agent_home"

  run_case "$RUN_DIR" "$dir/status_empty_json" "$BIN_PATH" agent status --config-root "$agent_home" --format json
  run_case "$RUN_DIR" "$dir/install_dry_run" "$BIN_PATH" agent install --config-root "$agent_home" --client claude-code --dry-run --format json
  if [ -e "$agent_home/.claude.json" ]; then
    printf 'present\n' > "$dir/dry_run.claude_exists"
  else
    printf 'absent\n' > "$dir/dry_run.claude_exists"
  fi

  printf '{"theme":"dark","mcpServers":{"other":{"command":"other-tool","args":["serve"]}}}\n' > "$agent_home/.claude.json"
  run_case "$RUN_DIR" "$dir/install_claude_preserve" "$BIN_PATH" agent install --config-root "$agent_home" --client claude-code --format json
  cp "$agent_home/.claude.json" "$dir/claude.after_install.json"
  sha256_file "$agent_home/.claude.json" > "$dir/claude.before_idempotent.sha256"

  run_case "$RUN_DIR" "$dir/install_claude_idempotent" "$BIN_PATH" agent install --config-root "$agent_home" --client claude-code --format json
  sha256_file "$agent_home/.claude.json" > "$dir/claude.after_idempotent.sha256"

  run_case_env "$RUN_DIR" "$dir/install_cursor_toon" "CODEINT_AGENT_HOME=$agent_home" "$BIN_PATH" agent install --client cursor --format toon
  if [ -f "$agent_home/.cursor/mcp.json" ]; then
    cp "$agent_home/.cursor/mcp.json" "$dir/cursor.mcp.json"
  fi

  run_case "$RUN_DIR" "$dir/install_codex_json" "$BIN_PATH" agent install --config-root "$agent_home" --client codex --format json
  if [ -f "$agent_home/.codex/config.toml" ]; then
    cp "$agent_home/.codex/config.toml" "$dir/codex.config.toml"
  fi

  run_case "$RUN_DIR" "$dir/status_registered_json" "$BIN_PATH" agent status --config-root "$agent_home" --format json

  local agent_bad="$RUN_DIR/work-agent-bad"
  rm -rf "$agent_bad"
  mkdir -p "$agent_bad"
  printf '{not-json\n' > "$agent_bad/.claude.json"
  sha256_file "$agent_bad/.claude.json" > "$dir/bad.before.sha256"
  run_case "$RUN_DIR" "$dir/invalid_json_config" "$BIN_PATH" agent install --config-root "$agent_bad" --client claude-code
  sha256_file "$agent_bad/.claude.json" > "$dir/bad.after.sha256"
}

drive_mcp() {
  local dir="$EVIDENCE_DIR/mcp"
  mkdir -p "$dir"

  local golden_in="$RUN_DIR/mcp-golden.jsonl"
  cat > "$golden_in" <<'EOF'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}
{"jsonrpc":"2.0","id":2,"method":"ping"}
{"jsonrpc":"2.0","id":3,"method":"tools/list"}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"check","arguments":{}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"guard","arguments":{"file":"src/sign.ts"}}}
EOF
  run_case_stdin "$FIXTURE_REPO" "$dir/session_golden" "$golden_in" "$BIN_PATH" mcp

  local work_base="$RUN_DIR/work-mcp-baseline"
  fresh_work_copy "$INIT_REPO" "$work_base"
  write_eroded_ts "$work_base/src/legacy.ts" "legacy" 14
  git -C "$work_base" add .
  git_commit_deterministic "$work_base" "2026-11-17T12:00:00Z" "Add legacy"
  run_case "$work_base" "$dir/seed_baseline" "$BIN_PATH" baseline update
  printf '{"budgets":{"production":{"erosion":0}}}\n' > "$work_base/codeint.json"

  local base_in="$RUN_DIR/mcp-baseline.jsonl"
  cat > "$base_in" <<'EOF'
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"check","arguments":{"ignore_baseline":false}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"check","arguments":{"ignore_baseline":true}}}
EOF
  run_case_stdin "$work_base" "$dir/check_ignore_baseline" "$base_in" "$BIN_PATH" mcp

  local err_in="$RUN_DIR/mcp-errors.jsonl"
  cat > "$err_in" <<'EOF'
{"jsonrpc":"2.0","id":1,"method":"unknown/method"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"no_such_tool","arguments":{}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"diff","arguments":{}}}
not-json
EOF
  run_case_stdin "$FIXTURE_REPO" "$dir/protocol_errors" "$err_in" "$BIN_PATH" mcp
}

drive_update() {
  local dir="$EVIDENCE_DIR/update"
  mkdir -p "$dir"

  run_case "$FIXTURE_REPO" "$dir/check_up_to_date" "$BIN_PATH" update --check --version "v${BIN_VERSION}"
  run_case "$FIXTURE_REPO" "$dir/check_available" "$BIN_PATH" update --check --version v99.0.0

  local local_rel="$RUN_DIR/work-update-local"
  rm -rf "$local_rel"
  mkdir -p "$local_rel"
  printf 'v99.0.0\n' > "$local_rel/LATEST"
  run_case_env "$FIXTURE_REPO" "$dir/local_latest_available" "CODEINT_RELEASE_BASE_URL=$local_rel" "$BIN_PATH" update --check
  run_case_env "$FIXTURE_REPO" "$dir/local_latest_missing" "CODEINT_RELEASE_BASE_URL=$RUN_DIR/work-update-missing" "$BIN_PATH" update --check

  local npm_root="${RUN_DIR:?}/work-update-npm"
  local npm_dir="$npm_root/node_modules/.bin"
  rm -rf "$npm_root"
  mkdir -p "$npm_dir"
  cp "$BIN_PATH" "$npm_dir/codeint"
  chmod +x "$npm_dir/codeint"
  run_case "$FIXTURE_REPO" "$dir/node_modules_refused" "$npm_dir/codeint" update --check --version "v${BIN_VERSION}"
}

is_valid_feature() {
  local candidate="$1"
  local f
  for f in "${ALL_FEATURES[@]}"; do
    if [ "$f" = "$candidate" ]; then
      return 0
    fi
  done
  return 1
}

drive_one() {
  if ! is_valid_feature "$1"; then
    printf 'unknown feature: %s\n' "$1" >&2
    exit 2
  fi
  "drive_${1//-/_}"
}

cmd_drive() {
  local run_dir="${1:?usage: verify.sh drive <run-dir> [all|<feature>]}"
  local feature="${2:-all}"
  load_manifest "$run_dir"

  if [ "$feature" = "all" ]; then
    local f
    for f in "${ALL_FEATURES[@]}"; do
      drive_one "$f"
    done
  else
    drive_one "$feature"
  fi
  printf 'drive: OK feature=%s evidence=%s\n' "$feature" "$EVIDENCE_DIR"
}

cmd_assert() {
  local run_dir="${1:?usage: verify.sh assert <run-dir> [all|<feature>]}"
  local feature="${2:-all}"
  load_manifest "$run_dir"

  local git_status
  git_status="$(git -C "$FIXTURE_REPO" status --porcelain)"
  local head_now
  head_now="$(git -C "$FIXTURE_REPO" rev-parse HEAD)"
  local cache_count
  cache_count="$(find "$CACHE_DIR" -type f | wc -l | tr -d ' ')"
  local bin_sha_now
  bin_sha_now="$(sha256_file "$BIN_PATH")"

  python3 - "$EVIDENCE_DIR" "$feature" "$BUILD_REV" "$RUN_ID" "$FIXTURE_HEAD" "$head_now" "$git_status" "$cache_count" "$BIN_SHA256" "$bin_sha_now" "$BIN_VERSION" <<'PY'
import json
import os
import sys

(
    evidence_dir,
    feature,
    build_rev,
    run_id,
    expected_head,
    actual_head,
    git_status,
    cache_count,
    expected_bin_sha,
    actual_bin_sha,
    bin_version,
) = sys.argv[1:12]

all_features = [
    "check",
    "diff",
    "trend",
    "init",
    "ratchet",
    "baseline",
    "plugin-test",
    "impact",
    "trace",
    "guard",
    "churn",
    "coupling",
    "report",
    "config",
    "schema",
    "doctor",
    "agent",
    "mcp",
    "update",
]

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

def read_jsonl(rel):
    t = read_text(rel)
    if t is None:
        return []
    out = []
    for line in t.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            out.append(json.loads(line))
        except Exception as e:
            out.append({"_parse_error": str(e), "_raw": line})
    return out

def setting_by_key(cfg_payload, key):
    if not isinstance(cfg_payload, dict):
        return None
    for item in cfg_payload.get("settings", []):
        if item.get("key") == key:
            return item
    return None

def check_by_id(report_payload, check_id):
    if not isinstance(report_payload, dict):
        return None
    for item in report_payload.get("checks", []):
        if item.get("id") == check_id:
            return item
    return None

features = all_features if feature == "all" else [feature]

check("fixture_repo_clean", git_status == "", f"git status not empty: {git_status!r}")
check("fixture_head_unchanged", actual_head == expected_head, f"expected={expected_head} actual={actual_head}")
check("binary_sha256_unchanged", actual_bin_sha == expected_bin_sha, f"expected={expected_bin_sha} actual={actual_bin_sha}")

if "check" in features:
    check("check.golden_toon.exit", read_exit("check/golden_toon") == 0, str(read_exit("check/golden_toon")))
    toon_out = read_text("check/golden_toon.stdout") or ""
    check("check.golden_toon.shape", "source_sets:" in toon_out and "skipped" in toon_out and "declaration_only" in toon_out, "missing expected TOON keys")

    check("check.golden_text.exit", read_exit("check/golden_text") == 0, str(read_exit("check/golden_text")))
    text_out = read_text("check/golden_text.stdout") or ""
    check("check.golden_text.shape", text_out.startswith("codeint check: PASS") and "Production" in text_out and "\x1b" not in text_out, text_out)

    check("check.config_format_text.exit", read_exit("check/config_format_text") == 0, str(read_exit("check/config_format_text")))
    cfg_text_out = read_text("check/config_format_text.stdout") or ""
    check("check.config_format_text.parity", cfg_text_out == text_out, "configured format text differed from --format text")

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

    check("diff.no_regression_text.exit", read_exit("diff/no_regression_text") == 0, str(read_exit("diff/no_regression_text")))
    diff_pass_text = read_text("diff/no_regression_text.stdout") or ""
    check("diff.no_regression_text.shape", diff_pass_text.startswith("codeint diff: PASS") and "\x1b" not in diff_pass_text, diff_pass_text)

    check("diff.regression_json.exit", read_exit("diff/regression_json") == 1, str(read_exit("diff/regression_json")))
    rj = read_json("diff/regression_json.stdout")
    check("diff.regression_json.payload", isinstance(rj, dict) and len(rj.get("regressions", [])) >= 1, str(rj))
    check("diff.regression_json.stderr", len((read_text("diff/regression_json.stderr") or "").strip()) > 0, "stderr empty on regression")

    check("diff.regression_text.exit", read_exit("diff/regression_text") == 1, str(read_exit("diff/regression_text")))
    diff_fail_text = read_text("diff/regression_text.stdout") or ""
    check("diff.regression_text.shape", diff_fail_text.startswith("codeint diff: FAIL") and "src/route.ts" in diff_fail_text and "\x1b" not in diff_fail_text, diff_fail_text)

    check("diff.regression_sarif.exit", read_exit("diff/regression_sarif") == 1, str(read_exit("diff/regression_sarif")))
    rs = read_json("diff/diff.sarif")
    check("diff.regression_sarif.file", isinstance(rs, dict) and rs.get("version") == "2.1.0" and len(rs.get("runs", [{}])[0].get("results", [])) >= 1, "diff.sarif missing new finding")

    check("diff.per_file_clean.exit", read_exit("diff/per_file_clean") == 0, str(read_exit("diff/per_file_clean")))
    pfc = read_json("diff/per_file_clean.stdout")
    check("diff.per_file_clean.payload", isinstance(pfc, dict) and not pfc.get("regressions"), str(pfc))

    check("diff.per_file_regression.exit", read_exit("diff/per_file_regression") == 1, str(read_exit("diff/per_file_regression")))
    pfr = read_json("diff/per_file_regression.stdout")
    pf_regs = pfr.get("regressions", []) if isinstance(pfr, dict) else []
    check("diff.per_file_regression.payload", any(r.get("file") == "src/sign.ts" and r.get("metric") == "erosion" for r in pf_regs), str(pf_regs))
    check("diff.per_file_regression.stderr", "file src/sign.ts regressed" in (read_text("diff/per_file_regression.stderr") or ""), read_text("diff/per_file_regression.stderr") or "")

    check("diff.new_file_budget_fail.exit", read_exit("diff/new_file_budget_fail") == 1, str(read_exit("diff/new_file_budget_fail")))
    nfb = read_json("diff/new_file_budget_fail.stdout")
    nfb_list = nfb.get("over_new_file_budget", []) if isinstance(nfb, dict) else []
    check("diff.new_file_budget_fail.payload", any(item.get("source_set") == "production" and item.get("metric") == "erosion" for item in nfb_list), str(nfb_list))
    check("diff.new_file_budget_sarif.exit", read_exit("diff/new_file_budget_sarif") == 1, str(read_exit("diff/new_file_budget_sarif")))
    nfb_sarif = read_json("diff/new_file_budget.sarif")
    nfb_props = nfb_sarif.get("runs", [{}])[0].get("properties", {}) if isinstance(nfb_sarif, dict) else {}
    check("diff.new_file_budget_sarif.props", len(nfb_props.get("newFileBudgetFailures", [])) >= 1, str(nfb_props))

    check("diff.new_file_budget_clean.exit", read_exit("diff/new_file_budget_clean") == 0, str(read_exit("diff/new_file_budget_clean")))
    nfbc = read_json("diff/new_file_budget_clean.stdout")
    check("diff.new_file_budget_clean.payload", isinstance(nfbc, dict) and not nfbc.get("over_new_file_budget"), str(nfbc))

    check("diff.new_file_budget_invalid.exit", read_exit("diff/new_file_budget_invalid") == 2, str(read_exit("diff/new_file_budget_invalid")))
    check("diff.new_file_budget_invalid.stderr", "orphan_files" in (read_text("diff/new_file_budget_invalid.stderr") or ""), read_text("diff/new_file_budget_invalid.stderr") or "")

    check("diff.bad_ref.exit", read_exit("diff/bad_ref") == 2, str(read_exit("diff/bad_ref")))
    check("diff.unborn_head.exit", read_exit("diff/unborn_head") == 2, str(read_exit("diff/unborn_head")))

if "trend" in features:
    check("trend.history_json.exit", read_exit("trend/history_json") == 0, str(read_exit("trend/history_json")))
    tj = read_json("trend/history_json.stdout")
    rows = tj.get("source_sets", {}).get("production", []) if isinstance(tj, dict) else []
    check("trend.history_json.commits", isinstance(tj, dict) and tj.get("commits") == 3 and len(rows) == 3, str(tj))
    check("trend.cache_populated", int(cache_count) >= 1, f"cache_count={cache_count}")

    check("trend.history_toon.exit", read_exit("trend/history_toon") == 0, str(read_exit("trend/history_toon")))
    check("trend.history_text.exit", read_exit("trend/history_text") == 0, str(read_exit("trend/history_text")))
    trend_text = read_text("trend/history_text.stdout") or ""
    check("trend.history_text.shape", trend_text.startswith("codeint trend") and "2026-11-02" in trend_text and "2026-11-16" in trend_text and "\x1b" not in trend_text, trend_text)
    check("trend.config_unsupported_format.exit", read_exit("trend/config_unsupported_format") == 2, str(read_exit("trend/config_unsupported_format")))
    trend_badfmt_err = read_text("trend/config_unsupported_format.stderr") or ""
    check("trend.config_unsupported_format.stderr", "trend" in trend_badfmt_err and "sarif" in trend_badfmt_err and "markdown" in trend_badfmt_err, trend_badfmt_err)
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
    check("init.adopt_diff_clean.exit", read_exit("init/adopt_diff_clean") == 0, str(read_exit("init/adopt_diff_clean")))
    check("init.adopt_check_new.exit", read_exit("init/adopt_check_new") == 1, str(read_exit("init/adopt_check_new")))
    adopt_new_err = read_text("init/adopt_check_new.stderr") or ""
    check("init.adopt_check_new.stderr", "production erosion ratio" in adopt_new_err and "is over its Budget 0" in adopt_new_err and "Ratchet" not in adopt_new_err, adopt_new_err)
    check("init.adopt_diff_new.exit", read_exit("init/adopt_diff_new") == 1, str(read_exit("init/adopt_diff_new")))
    adopt_diff_new_err = read_text("init/adopt_diff_new.stderr") or ""
    check("init.adopt_diff_new.stderr", "the non-baselined production erosion ratio" in adopt_diff_new_err and "is over its Budget 0" in adopt_diff_new_err, adopt_diff_new_err)

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

if "ratchet" in features:
    check("ratchet.create_json.exit", read_exit("ratchet/create_json") == 0, str(read_exit("ratchet/create_json")))
    rc = read_json("ratchet/create_json.stdout")
    check("ratchet.create_json.payload", isinstance(rc, dict) and rc.get("created") is True and rc.get("total_entries", 0) >= 5, str(rc))
    rf_created = read_json("ratchet/ratchet.created.json")
    rf_erosion = rf_created.get("source_sets", {}).get("production", {}).get("erosion", {}) if isinstance(rf_created, dict) else {}
    created_ceiling = rf_erosion.get("ceiling", 0)
    check("ratchet.create_json.file", isinstance(rf_created, dict) and rf_created.get("schema_version") == 1 and created_ceiling > 0 and rf_erosion.get("metric_version", 0) >= 1, str(rf_created))

    check("ratchet.check_pass.exit", read_exit("ratchet/check_pass") == 0, str(read_exit("ratchet/check_pass")))
    check("ratchet.check_fail.exit", read_exit("ratchet/check_fail") == 1, str(read_exit("ratchet/check_fail")))
    rcf = read_json("ratchet/check_fail.stdout")
    check("ratchet.check_fail.payload", isinstance(rcf, dict) and any(item.get("metric") == "erosion" for item in rcf.get("over_ratchet", [])), str(rcf))
    check("ratchet.check_fail.stderr", "Ratchet" in (read_text("ratchet/check_fail.stderr") or ""), read_text("ratchet/check_fail.stderr") or "")

    check("ratchet.diff_fail.exit", read_exit("ratchet/diff_fail") == 1, str(read_exit("ratchet/diff_fail")))
    rdf = read_json("ratchet/diff_fail.stdout")
    check("ratchet.diff_fail.payload", isinstance(rdf, dict) and any(item.get("metric") == "erosion" for item in rdf.get("over_ratchet", [])), str(rdf))
    check("ratchet.diff_fail.stderr", "Ratchet" in (read_text("ratchet/diff_fail.stderr") or ""), read_text("ratchet/diff_fail.stderr") or "")

    check("ratchet.loosen_refused.exit", read_exit("ratchet/loosen_refused") == 2, str(read_exit("ratchet/loosen_refused")))
    check("ratchet.loosen_refused.stderr", "refusing to loosen Ratchet ceilings" in (read_text("ratchet/loosen_refused.stderr") or ""), read_text("ratchet/loosen_refused.stderr") or "")
    rb_sha = (read_text("ratchet/ratchet.before_loosen.sha256") or "").strip()
    ra_sha = (read_text("ratchet/ratchet.after_loosen_refused.sha256") or "").strip()
    check("ratchet.loosen_refused.unchanged", rb_sha != "" and rb_sha == ra_sha, f"before={rb_sha} after={ra_sha}")

    check("ratchet.loosen_reset.exit", read_exit("ratchet/loosen_reset") == 0, str(read_exit("ratchet/loosen_reset")))
    check("ratchet.loosen_reset.toon", "reset: true" in (read_text("ratchet/loosen_reset.stdout") or ""), read_text("ratchet/loosen_reset.stdout") or "")
    rf_reset = read_json("ratchet/ratchet.reset.json")
    reset_ceiling = rf_reset.get("source_sets", {}).get("production", {}).get("erosion", {}).get("ceiling", 0) if isinstance(rf_reset, dict) else 0
    check("ratchet.loosen_reset.file", reset_ceiling > created_ceiling, f"created={created_ceiling} reset={reset_ceiling}")

    check("ratchet.tighten.exit", read_exit("ratchet/tighten") == 0, str(read_exit("ratchet/tighten")))
    rt = read_json("ratchet/tighten.stdout")
    check("ratchet.tighten.payload", isinstance(rt, dict) and rt.get("tightened", 0) >= 1, str(rt))
    rf_tight = read_json("ratchet/ratchet.tightened.json")
    tight_ceiling = rf_tight.get("source_sets", {}).get("production", {}).get("erosion", {}).get("ceiling", 1) if isinstance(rf_tight, dict) else 1
    check("ratchet.tighten.file", tight_ceiling < reset_ceiling, f"reset={reset_ceiling} tight={tight_ceiling}")

    check("ratchet.version_mismatch.exit", read_exit("ratchet/version_mismatch") == 2, str(read_exit("ratchet/version_mismatch")))
    check("ratchet.version_mismatch.stderr", "codeint ratchet update" in (read_text("ratchet/version_mismatch.stderr") or ""), read_text("ratchet/version_mismatch.stderr") or "")

    check("ratchet.corrupt_refused.exit", read_exit("ratchet/corrupt_refused") == 2, str(read_exit("ratchet/corrupt_refused")))
    check("ratchet.corrupt_reset.exit", read_exit("ratchet/corrupt_reset") == 0, str(read_exit("ratchet/corrupt_reset")))
    rcr = read_json("ratchet/corrupt_reset.stdout")
    rf_rec = read_json("ratchet/ratchet.recovered.json")
    check("ratchet.corrupt_reset.payload", isinstance(rcr, dict) and rcr.get("replaced_invalid") is True and isinstance(rf_rec, dict) and rf_rec.get("schema_version") == 1, str(rcr))

    check("ratchet.package_skipped.exit", read_exit("ratchet/package_skipped") == 0, str(read_exit("ratchet/package_skipped")))
    rps = read_json("ratchet/package_skipped.stdout")
    check("ratchet.package_skipped.note", isinstance(rps, dict) and bool(rps.get("ratchet_note")), str(rps))

if "baseline" in features:
    check("baseline.create_json.exit", read_exit("baseline/create_json") == 0, str(read_exit("baseline/create_json")))
    bc = read_json("baseline/create_json.stdout")
    check("baseline.create_json.payload", isinstance(bc, dict) and bc.get("created") is True and bc.get("added", 0) >= 1, str(bc))
    bf_created = read_json("baseline/baseline.created.json")
    bf_created_ids = bf_created.get("source_sets", {}).get("production", {}).get("erosion", {}).get("findings", []) if isinstance(bf_created, dict) else []
    check("baseline.create_json.file", isinstance(bf_created, dict) and bf_created.get("schema_version") == 2 and len(bf_created_ids) >= 1, str(bf_created))

    check("baseline.check_hides.exit", read_exit("baseline/check_hides") == 0, str(read_exit("baseline/check_hides")))
    bch = read_json("baseline/check_hides.stdout")
    bch_meta = bch.get("baseline", {}) if isinstance(bch, dict) else {}
    bch_erosion = bch.get("source_sets", {}).get("production", {}).get("erosion", {}) if isinstance(bch, dict) else {}
    check("baseline.check_hides.payload", bch_meta.get("path") == ".codeint/baseline.json" and bch_meta.get("hidden_findings", 0) >= 1 and bch_erosion.get("baselined_findings", 0) >= 1 and not bch.get("over_budget"), str(bch))

    check("baseline.check_ignore_baseline.exit", read_exit("baseline/check_ignore_baseline") == 1, str(read_exit("baseline/check_ignore_baseline")))
    bci = read_json("baseline/check_ignore_baseline.stdout")
    check("baseline.check_ignore_baseline.payload", isinstance(bci, dict) and "baseline" not in bci and any(item.get("metric") == "erosion" for item in bci.get("over_budget", [])), str(bci))
    check("baseline.check_ignore_baseline.stderr", "Budget" in (read_text("baseline/check_ignore_baseline.stderr") or ""), read_text("baseline/check_ignore_baseline.stderr") or "")

    check("baseline.prune_and_skip_new.exit", read_exit("baseline/prune_and_skip_new") == 0, str(read_exit("baseline/prune_and_skip_new")))
    bp = read_json("baseline/prune_and_skip_new.stdout")
    bf_pruned = read_json("baseline/baseline.pruned.json")
    bf_pruned_ids = bf_pruned.get("source_sets", {}).get("production", {}).get("erosion", {}).get("findings", []) if isinstance(bf_pruned, dict) else []
    check("baseline.prune_and_skip_new.payload", isinstance(bp, dict) and bp.get("removed", 0) >= 1 and bp.get("skipped_new", 0) >= 1 and len(bf_pruned_ids) == 0, str(bp))

    check("baseline.reset_adds_new.exit", read_exit("baseline/reset_adds_new") == 0, str(read_exit("baseline/reset_adds_new")))
    bf_reset = read_json("baseline/baseline.reset.json")
    bf_reset_ids = bf_reset.get("source_sets", {}).get("production", {}).get("erosion", {}).get("findings", []) if isinstance(bf_reset, dict) else []
    check("baseline.reset_adds_new.file", len(bf_reset_ids) >= 1 and bf_reset_ids != bf_created_ids, str(bf_reset_ids))
    check("baseline.reset_check_pass.exit", read_exit("baseline/reset_check_pass") == 0, str(read_exit("baseline/reset_check_pass")))

    check("baseline.version_mismatch.exit", read_exit("baseline/version_mismatch") == 2, str(read_exit("baseline/version_mismatch")))
    check("baseline.version_mismatch.stderr", "codeint baseline update" in (read_text("baseline/version_mismatch.stderr") or ""), read_text("baseline/version_mismatch.stderr") or "")

    check("baseline.corrupt_refused.exit", read_exit("baseline/corrupt_refused") == 2, str(read_exit("baseline/corrupt_refused")))
    check("baseline.corrupt_reset.exit", read_exit("baseline/corrupt_reset") == 0, str(read_exit("baseline/corrupt_reset")))
    bcr = read_json("baseline/corrupt_reset.stdout")
    bf_rec = read_json("baseline/baseline.recovered.json")
    check("baseline.corrupt_reset.payload", isinstance(bcr, dict) and bcr.get("replaced_invalid") is True and isinstance(bf_rec, dict) and bf_rec.get("schema_version") == 2, str(bcr))

if "plugin-test" in features:
    check("plugin-test.language_json.exit", read_exit("plugin-test/language_json") == 0, str(read_exit("plugin-test/language_json")))
    pl = read_json("plugin-test/language_json.stdout")
    pl_e2e = check_by_id(pl, "end-to-end")
    check("plugin-test.language_json.payload", isinstance(pl, dict) and pl.get("passed") is True and pl.get("kind") == "language" and pl.get("summary", {}).get("failed") == 0 and isinstance(pl_e2e, dict) and pl_e2e.get("status") == "pass", str(pl))

    check("plugin-test.language_source_text.exit", read_exit("plugin-test/language_source_text") == 0, str(read_exit("plugin-test/language_source_text")))
    check("plugin-test.language_source_text.shape", len((read_text("plugin-test/language_source_text.stdout") or "").strip()) > 0, "empty text output")

    check("plugin-test.metric_toon.exit", read_exit("plugin-test/metric_toon") == 0, str(read_exit("plugin-test/metric_toon")))
    pm_toon = read_text("plugin-test/metric_toon.stdout") or ""
    check("plugin-test.metric_toon.shape", "passed: true" in pm_toon and "kind: metric" in pm_toon, pm_toon)

    check("plugin-test.contract_fail_json.exit", read_exit("plugin-test/contract_fail_json") == 1, str(read_exit("plugin-test/contract_fail_json")))
    pf = read_json("plugin-test/contract_fail_json.stdout")
    pf_bounds = check_by_id(pf, "measurement-bounds")
    check("plugin-test.contract_fail_json.payload", isinstance(pf, dict) and pf.get("passed") is False and isinstance(pf_bounds, dict) and pf_bounds.get("status") == "fail" and bool(pf_bounds.get("hint")), str(pf))
    check("plugin-test.contract_fail_json.stderr", "measurement-bounds" in (read_text("plugin-test/contract_fail_json.stderr") or ""), read_text("plugin-test/contract_fail_json.stderr") or "")

    check("plugin-test.corrupt_wasm.exit", read_exit("plugin-test/corrupt_wasm") == 1, str(read_exit("plugin-test/corrupt_wasm")))
    pc = read_json("plugin-test/corrupt_wasm.stdout")
    pc_comp = check_by_id(pc, "component")
    check("plugin-test.corrupt_wasm.payload", isinstance(pc, dict) and pc.get("passed") is False and isinstance(pc_comp, dict) and pc_comp.get("status") == "fail" and bool(pc_comp.get("hint")), str(pc))

    check("plugin-test.missing_file.exit", read_exit("plugin-test/missing_file") == 2, str(read_exit("plugin-test/missing_file")))

if "impact" in features:
    check("impact.direct_and_transitive_json.exit", read_exit("impact/direct_and_transitive_json") == 0, str(read_exit("impact/direct_and_transitive_json")))
    ij = read_json("impact/direct_and_transitive_json.stdout")
    prod_direct = [item.get("path") for item in ij.get("production", {}).get("direct", [])] if isinstance(ij, dict) else []
    prod_trans = [item.get("path") for item in ij.get("production", {}).get("transitive", [])] if isinstance(ij, dict) else []
    test_direct = [item.get("path") for item in ij.get("test", {}).get("direct", [])] if isinstance(ij, dict) else []
    check("impact.direct_and_transitive_json.payload", isinstance(ij, dict) and ij.get("summary", {}).get("direct_dependents") == 2 and ij.get("summary", {}).get("transitive_dependents") == 1 and "src/b.ts" in prod_direct and "src/c.ts" in prod_trans and "src/a.test.ts" in test_direct, str(ij))

    check("impact.depth_one_text.exit", read_exit("impact/depth_one_text") == 0, str(read_exit("impact/depth_one_text")))
    d1_text = read_text("impact/depth_one_text.stdout") or ""
    check("impact.depth_one_text.shape", "src/b.ts" in d1_text and "src/c.ts" not in d1_text, d1_text)

    check("impact.changed_toon.exit", read_exit("impact/changed_toon") == 0, str(read_exit("impact/changed_toon")))
    ch_toon = read_text("impact/changed_toon.stdout") or ""
    check("impact.changed_toon.shape", "src/a.ts" in ch_toon and "src/b.ts" in ch_toon, ch_toon)

    check("impact.no_dependents_output.exit", read_exit("impact/no_dependents_output") == 0, str(read_exit("impact/no_dependents_output")))
    check("impact.no_dependents_output.stdout_empty", (read_text("impact/no_dependents_output.stdout") or "") == "", "stdout not empty with --output")
    imp_file = read_json("impact/impact.json")
    check("impact.no_dependents_output.file", isinstance(imp_file, dict) and imp_file.get("summary", {}).get("total_dependents") == 0, str(imp_file))

    check("impact.missing_package.exit", read_exit("impact/missing_package") == 2, str(read_exit("impact/missing_package")))
    check("impact.missing_file.exit", read_exit("impact/missing_file") == 2, str(read_exit("impact/missing_file")))
    check("impact.non_repo.exit", read_exit("impact/non_repo") == 2, str(read_exit("impact/non_repo")))

if "trace" in features:
    check("trace.connected_json.exit", read_exit("trace/connected_json") == 0, str(read_exit("trace/connected_json")))
    tc = read_json("trace/connected_json.stdout")
    check("trace.connected_json.payload", isinstance(tc, dict) and tc.get("connected") is True and tc.get("shortest_distance") == 2 and len(tc.get("paths", [])) == 1, str(tc))

    check("trace.all_paths_json.exit", read_exit("trace/all_paths_json") == 0, str(read_exit("trace/all_paths_json")))
    ta = read_json("trace/all_paths_json.stdout")
    check("trace.all_paths_json.payload", isinstance(ta, dict) and ta.get("connected") is True and len(ta.get("paths", [])) == 2 and ta.get("truncated") is False, str(ta))

    check("trace.disconnected_toon.exit", read_exit("trace/disconnected_toon") == 0, str(read_exit("trace/disconnected_toon")))
    td_toon = read_text("trace/disconnected_toon.stdout") or ""
    check("trace.disconnected_toon.shape", "connected: false" in td_toon and "paths: []" in td_toon, td_toon)

    check("trace.text_output.exit", read_exit("trace/text_output") == 0, str(read_exit("trace/text_output")))
    check("trace.text_output.stdout_empty", (read_text("trace/text_output.stdout") or "") == "", "stdout not empty with --output")
    tr_txt = read_text("trace/trace.txt") or ""
    check("trace.text_output.file", "src/entry.ts" in tr_txt and "src/leaf.ts" in tr_txt, tr_txt)

    check("trace.missing_package.exit", read_exit("trace/missing_package") == 2, str(read_exit("trace/missing_package")))
    check("trace.unknown_file.exit", read_exit("trace/unknown_file") == 2, str(read_exit("trace/unknown_file")))
    check("trace.non_repo.exit", read_exit("trace/non_repo") == 2, str(read_exit("trace/non_repo")))

if "guard" in features:
    check("guard.unconfigured_json.exit", read_exit("guard/unconfigured_json") == 0, str(read_exit("guard/unconfigured_json")))
    gu = read_json("guard/unconfigured_json.stdout")
    check("guard.unconfigured_json.payload", isinstance(gu, dict) and gu.get("configured") is False and gu.get("exists") is True, str(gu))

    check("guard.configured_existing_json.exit", read_exit("guard/configured_existing_json") == 0, str(read_exit("guard/configured_existing_json")))
    ge = read_json("guard/configured_existing_json.stdout")
    denied_zones = [item.get("zone") for item in ge.get("denied_targets", [])] if isinstance(ge, dict) else []
    check("guard.configured_existing_json.payload", isinstance(ge, dict) and ge.get("configured") is True and ge.get("exists") is True and ge.get("zone") == "domain" and "ui" in denied_zones and ge.get("external_imports", {}).get("directive") == "deny_external", str(ge))

    check("guard.configured_planned_toon.exit", read_exit("guard/configured_planned_toon") == 0, str(read_exit("guard/configured_planned_toon")))
    gp = read_text("guard/configured_planned_toon.stdout") or ""
    check("guard.configured_planned_toon.shape", "exists: false" in gp and "zone: ui" in gp and "domain" in gp, gp)

    check("guard.configured_text.exit", read_exit("guard/configured_text") == 0, str(read_exit("guard/configured_text")))
    gt = read_text("guard/configured_text.stdout") or ""
    check("guard.configured_text.shape", "domain" in gt and "ui" in gt, gt)

    check("guard.outside_repo.exit", read_exit("guard/outside_repo") == 2, str(read_exit("guard/outside_repo")))
    check("guard.non_repo.exit", read_exit("guard/non_repo") == 2, str(read_exit("guard/non_repo")))

if "churn" in features:
    check("churn.commits_json.exit", read_exit("churn/commits_json") == 0, str(read_exit("churn/commits_json")))
    chj = read_json("churn/commits_json.stdout")
    ch_findings = chj.get("source_sets", {}).get("production", {}).get("erosion", {}).get("findings", []) if isinstance(chj, dict) else []
    check("churn.commits_json.payload", isinstance(chj, dict) and chj.get("commits_analyzed", 0) >= 3 and any(f.get("path") == "src/hot.ts" and f.get("commits", 0) >= 3 for f in ch_findings), str(chj))

    check("churn.since_toon.exit", read_exit("churn/since_toon") == 0, str(read_exit("churn/since_toon")))
    chst = read_text("churn/since_toon.stdout") or ""
    check("churn.since_toon.shape", "commits_analyzed:" in chst and "source_sets:" in chst, chst)

    check("churn.text_all.exit", read_exit("churn/text_all") == 0, str(read_exit("churn/text_all")))
    check("churn.text_all.shape", "src/hot.ts" in (read_text("churn/text_all.stdout") or ""), read_text("churn/text_all.stdout") or "")

    check("churn.markdown_output.exit", read_exit("churn/markdown_output") == 0, str(read_exit("churn/markdown_output")))
    check("churn.markdown_output.stdout_empty", (read_text("churn/markdown_output.stdout") or "") == "", "stdout not empty with --output")
    check("churn.markdown_output.file", len(read_text("churn/churn.md") or "") > 0, "churn.md empty")

    check("churn.bad_since.exit", read_exit("churn/bad_since") == 2, str(read_exit("churn/bad_since")))
    check("churn.unborn_head.exit", read_exit("churn/unborn_head") == 2, str(read_exit("churn/unborn_head")))

if "coupling" in features:
    check("coupling.pairs_json.exit", read_exit("coupling/pairs_json") == 0, str(read_exit("coupling/pairs_json")))
    cpj = read_json("coupling/pairs_json.stdout")
    pairs = cpj.get("pairs", []) if isinstance(cpj, dict) else []
    has_ab = any({p.get("file_a"), p.get("file_b")} == {"src/alpha.ts", "src/beta.ts"} and p.get("co_change_count", 0) >= 3 and p.get("has_direct_import") is False for p in pairs)
    check("coupling.pairs_json.payload", isinstance(cpj, dict) and cpj.get("source_sets") == ["production"] and has_ab, str(cpj))

    check("coupling.bulk_excluded_json.exit", read_exit("coupling/bulk_excluded_json") == 0, str(read_exit("coupling/bulk_excluded_json")))
    cbj = read_json("coupling/bulk_excluded_json.stdout")
    check("coupling.bulk_excluded_json.payload", isinstance(cbj, dict) and cbj.get("commits_excluded_by_bulk_limit", 0) >= 1, str(cbj))

    check("coupling.include_tests_toon.exit", read_exit("coupling/include_tests_toon") == 0, str(read_exit("coupling/include_tests_toon")))
    cit = read_text("coupling/include_tests_toon.stdout") or ""
    check("coupling.include_tests_toon.shape", "test" in cit and "src/alpha.test.ts" in cit, cit)

    check("coupling.markdown_output.exit", read_exit("coupling/markdown_output") == 0, str(read_exit("coupling/markdown_output")))
    check("coupling.markdown_output.stdout_empty", (read_text("coupling/markdown_output.stdout") or "") == "", "stdout not empty with --output")
    check("coupling.markdown_output.file", len(read_text("coupling/coupling.md") or "") > 0, "coupling.md empty")

    check("coupling.text_format.exit", read_exit("coupling/text_format") == 0, str(read_exit("coupling/text_format")))
    ctf = read_text("coupling/text_format.stdout") or ""
    check("coupling.text_format.shape", "src/alpha.ts" in ctf and "src/beta.ts" in ctf, ctf)

    check("coupling.invalid_ratio.exit", read_exit("coupling/invalid_ratio") == 2, str(read_exit("coupling/invalid_ratio")))
    check("coupling.unborn_head.exit", read_exit("coupling/unborn_head") == 2, str(read_exit("coupling/unborn_head")))

if "report" in features:
    check("report.check_to_sarif_output.exit", read_exit("report/check_to_sarif_output") == 0, str(read_exit("report/check_to_sarif_output")))
    check("report.check_to_sarif_output.stdout_empty", (read_text("report/check_to_sarif_output.stdout") or "") == "", "stdout not empty with --output")
    rsf = read_json("report/report.sarif")
    check("report.check_to_sarif_output.file", isinstance(rsf, dict) and rsf.get("version") == "2.1.0" and rsf.get("runs", [{}])[0].get("tool", {}).get("driver", {}).get("name") == "codeint", str(rsf))

    check("report.check_to_markdown.exit", read_exit("report/check_to_markdown") == 0, str(read_exit("report/check_to_markdown")))
    rmd = read_text("report/check_to_markdown.stdout") or ""
    check("report.check_to_markdown.shape", "codeint report" in rmd and "| Source set |" in rmd, rmd)

    check("report.check_to_text_stdin.exit", read_exit("report/check_to_text_stdin") == 0, str(read_exit("report/check_to_text_stdin")))
    rtxt = read_text("report/check_to_text_stdin.stdout") or ""
    check("report.check_to_text_stdin.shape", "codeint check: PASS" in rtxt and "Metric" in rtxt and "\x1b[" not in rtxt, rtxt)

    check("report.check_to_text_all.exit", read_exit("report/check_to_text_all") == 0, str(read_exit("report/check_to_text_all")))
    rtxt_all = read_text("report/check_to_text_all.stdout") or ""
    check("report.check_to_text_all.shape", "codeint check: PASS" in rtxt_all and "Metric" in rtxt_all and "\x1b[" not in rtxt_all, rtxt_all)

    check("report.diff_to_toon.exit", read_exit("report/diff_to_toon") == 0, str(read_exit("report/diff_to_toon")))
    check("report.diff_to_toon.shape", len((read_text("report/diff_to_toon.stdout") or "").strip()) > 0, "empty diff toon")

    check("report.diff_to_json.exit", read_exit("report/diff_to_json") == 0, str(read_exit("report/diff_to_json")))
    rdj = read_json("report/diff_to_json.stdout")
    check("report.diff_to_json.payload", isinstance(rdj, dict) and "_parse_error" not in rdj and "source_sets" in rdj, str(rdj))

    check("report.missing_input.exit", read_exit("report/missing_input") == 2, str(read_exit("report/missing_input")))
    check("report.malformed_json.exit", read_exit("report/malformed_json") == 2, str(read_exit("report/malformed_json")))
    check("report.unsupported_schema.exit", read_exit("report/unsupported_schema") == 2, str(read_exit("report/unsupported_schema")))

if "config" in features:
    check("config.resolved_default_json.exit", read_exit("config/resolved_default_json") == 0, str(read_exit("config/resolved_default_json")))
    cfd = read_json("config/resolved_default_json.stdout")
    check("config.resolved_default_json.payload", isinstance(cfd, dict) and cfd.get("config_file_present") is False and all(s.get("source") == "default" for s in cfd.get("settings", [])), str(cfd))

    check("config.resolved_file_and_cli_json.exit", read_exit("config/resolved_file_and_cli_json") == 0, str(read_exit("config/resolved_file_and_cli_json")))
    cfx = read_json("config/resolved_file_and_cli_json.stdout")
    s_pf = setting_by_key(cfx, "regression_limits.per_file")
    s_bud = setting_by_key(cfx, "budgets.production.erosion")
    s_mem = setting_by_key(cfx, "plugin_limits.memory_mb")
    s_tmo = setting_by_key(cfx, "plugin_limits.timeout_ms")
    check("config.resolved_file_and_cli_json.payload", isinstance(cfx, dict) and cfx.get("config_file_present") is True and s_pf and s_pf.get("source") == "file" and s_pf.get("value") is True and s_bud and s_bud.get("source") == "cli" and s_bud.get("value") == 0.25 and s_mem and s_mem.get("source") == "cli" and s_mem.get("value") == 128 and s_tmo and s_tmo.get("source") == "cli" and s_tmo.get("value") == 10000, str((s_pf, s_bud, s_mem, s_tmo)))

    check("config.resolved_subfolder_text.exit", read_exit("config/resolved_subfolder_text") == 0, str(read_exit("config/resolved_subfolder_text")))
    check("config.resolved_subfolder_text.shape", len((read_text("config/resolved_subfolder_text.stdout") or "").strip()) > 0, "empty text output")

    check("config.resolved_toon.exit", read_exit("config/resolved_toon") == 0, str(read_exit("config/resolved_toon")))
    check("config.resolved_toon.shape", "settings" in (read_text("config/resolved_toon.stdout") or ""), read_text("config/resolved_toon.stdout") or "")

    check("config.invalid_config.exit", read_exit("config/invalid_config") == 2, str(read_exit("config/invalid_config")))
    check("config.invalid_override.exit", read_exit("config/invalid_override") == 2, str(read_exit("config/invalid_override")))
    check("config.non_repo.exit", read_exit("config/non_repo") == 2, str(read_exit("config/non_repo")))

if "schema" in features:
    check("schema.json_default.exit", read_exit("schema/json_default") == 0, str(read_exit("schema/json_default")))
    sc_raw = read_text("schema/json_default.stdout")
    sc = read_json("schema/json_default.stdout")
    sc_cmds = {c.get("name") for c in sc.get("commands", [])} if isinstance(sc, dict) else set()
    sc_tools = {t.get("name") for t in sc.get("mcp_tools", [])} if isinstance(sc, dict) else set()
    expected_cmds = {"check", "diff", "trend", "init", "ratchet", "baseline", "plugin", "impact", "trace", "guard", "churn", "coupling", "report", "config", "schema", "doctor", "agent", "mcp", "update"}
    expected_tools = {"check", "diff", "trend", "guard", "impact", "trace", "churn", "coupling"}
    check("schema.json_default.payload", isinstance(sc, dict) and sc.get("name") == "codeint" and sc.get("version") == bin_version and expected_cmds.issubset(sc_cmds) and expected_tools == sc_tools and len(sc.get("languages", [])) >= 8 and len(sc.get("metrics", [])) >= 8, str((sc_cmds, sc_tools)))

    check("schema.toon_format.exit", read_exit("schema/toon_format") == 0, str(read_exit("schema/toon_format")))
    sc_toon = read_text("schema/toon_format.stdout") or ""
    check("schema.toon_format.shape", "name: codeint" in sc_toon and "commands" in sc_toon, sc_toon[:200])

    check("schema.output_file.exit", read_exit("schema/output_file") == 0, str(read_exit("schema/output_file")))
    check("schema.output_file.stdout_empty", (read_text("schema/output_file.stdout") or "") == "", "stdout not empty with --output")
    check("schema.output_file.deterministic", sc_raw is not None and read_text("schema/schema.json") == sc_raw, "schema.json differed from stdout")

    check("schema.non_repo.exit", read_exit("schema/non_repo") == 0, str(read_exit("schema/non_repo")))
    check("schema.non_repo.deterministic", sc_raw is not None and read_text("schema/non_repo.stdout") == sc_raw, "non_repo schema differed")

    check("schema.unknown_option.exit", read_exit("schema/unknown_option") == 2, str(read_exit("schema/unknown_option")))

if "doctor" in features:
    check("doctor.healthy_with_config_json.exit", read_exit("doctor/healthy_with_config_json") == 0, str(read_exit("doctor/healthy_with_config_json")))
    dh = read_json("doctor/healthy_with_config_json.stdout")
    check("doctor.healthy_with_config_json.payload", isinstance(dh, dict) and dh.get("status") == "pass" and dh.get("summary", {}).get("errors") == 0 and dh.get("summary", {}).get("warnings") == 0, str(dh))

    check("doctor.no_config_warn_text.exit", read_exit("doctor/no_config_warn_text") == 0, str(read_exit("doctor/no_config_warn_text")))
    dw = read_text("doctor/no_config_warn_text.stdout") or ""
    check("doctor.no_config_warn_text.shape", "warn" in dw.lower() or "no configuration" in dw.lower(), dw)

    check("doctor.subfolder_toon.exit", read_exit("doctor/subfolder_toon") == 0, str(read_exit("doctor/subfolder_toon")))
    dt = read_text("doctor/subfolder_toon.stdout") or ""
    check("doctor.subfolder_toon.shape", "status:" in dt and "checks" in dt, dt)

    check("doctor.invalid_config_fail.exit", read_exit("doctor/invalid_config_fail") == 1, str(read_exit("doctor/invalid_config_fail")))
    dic = read_json("doctor/invalid_config_fail.stdout")
    dic_cfg = check_by_id(dic, "config")
    check("doctor.invalid_config_fail.payload", isinstance(dic, dict) and dic.get("status") == "error" and dic.get("summary", {}).get("errors", 0) >= 1 and isinstance(dic_cfg, dict) and dic_cfg.get("status") == "error", str(dic))

    check("doctor.missing_plugin_fail.exit", read_exit("doctor/missing_plugin_fail") == 1, str(read_exit("doctor/missing_plugin_fail")))
    dmp = read_json("doctor/missing_plugin_fail.stdout")
    dmp_plg = check_by_id(dmp, "plugin:missing")
    check("doctor.missing_plugin_fail.payload", isinstance(dmp, dict) and dmp.get("status") == "error" and isinstance(dmp_plg, dict) and dmp_plg.get("status") == "error", str(dmp))

    check("doctor.non_repo_warn.exit", read_exit("doctor/non_repo_warn") == 0, str(read_exit("doctor/non_repo_warn")))
    dnr = read_json("doctor/non_repo_warn.stdout")
    dnr_git = check_by_id(dnr, "git")
    check("doctor.non_repo_warn.payload", isinstance(dnr, dict) and dnr.get("status") == "warn" and dnr.get("summary", {}).get("errors") == 0 and isinstance(dnr_git, dict) and dnr_git.get("status") == "warn", str(dnr))

if "agent" in features:
    check("agent.status_empty_json.exit", read_exit("agent/status_empty_json") == 0, str(read_exit("agent/status_empty_json")))
    ase = read_json("agent/status_empty_json.stdout")
    ase_agents = ase.get("agents", []) if isinstance(ase, dict) else []
    check("agent.status_empty_json.payload", len(ase_agents) == 3 and all(a.get("registered") is False for a in ase_agents), str(ase))

    check("agent.install_dry_run.exit", read_exit("agent/install_dry_run") == 0, str(read_exit("agent/install_dry_run")))
    adr = read_json("agent/install_dry_run.stdout")
    check("agent.install_dry_run.payload", isinstance(adr, dict) and adr.get("dry_run") is True and adr.get("results", [{}])[0].get("written") is False and (read_text("agent/dry_run.claude_exists") or "").strip() == "absent", str(adr))

    check("agent.install_claude_preserve.exit", read_exit("agent/install_claude_preserve") == 0, str(read_exit("agent/install_claude_preserve")))
    acp = read_json("agent/install_claude_preserve.stdout")
    claude_file = read_json("agent/claude.after_install.json")
    check("agent.install_claude_preserve.payload", isinstance(acp, dict) and acp.get("results", [{}])[0].get("written") is True and isinstance(claude_file, dict) and claude_file.get("theme") == "dark" and "other" in claude_file.get("mcpServers", {}) and claude_file.get("mcpServers", {}).get("codeint") == {"command": "codeint", "args": ["mcp"]}, str((acp, claude_file)))

    check("agent.install_claude_idempotent.exit", read_exit("agent/install_claude_idempotent") == 0, str(read_exit("agent/install_claude_idempotent")))
    aci = read_json("agent/install_claude_idempotent.stdout")
    ab_sha = (read_text("agent/claude.before_idempotent.sha256") or "").strip()
    aa_sha = (read_text("agent/claude.after_idempotent.sha256") or "").strip()
    check("agent.install_claude_idempotent.payload", isinstance(aci, dict) and aci.get("results", [{}])[0].get("action") == "unchanged" and aci.get("results", [{}])[0].get("written") is False and ab_sha != "" and ab_sha == aa_sha, str(aci))

    check("agent.install_cursor_toon.exit", read_exit("agent/install_cursor_toon") == 0, str(read_exit("agent/install_cursor_toon")))
    cursor_file = read_json("agent/cursor.mcp.json")
    check("agent.install_cursor_toon.file", isinstance(cursor_file, dict) and cursor_file.get("mcpServers", {}).get("codeint") == {"command": "codeint", "args": ["mcp"]}, str(cursor_file))

    check("agent.install_codex_json.exit", read_exit("agent/install_codex_json") == 0, str(read_exit("agent/install_codex_json")))
    codex_file = read_text("agent/codex.config.toml") or ""
    check("agent.install_codex_json.file", "mcp_servers.codeint" in codex_file and "codeint" in codex_file and "mcp" in codex_file, codex_file)

    check("agent.status_registered_json.exit", read_exit("agent/status_registered_json") == 0, str(read_exit("agent/status_registered_json")))
    asr = read_json("agent/status_registered_json.stdout")
    asr_agents = asr.get("agents", []) if isinstance(asr, dict) else []
    check("agent.status_registered_json.payload", len(asr_agents) == 3 and all(a.get("registered") is True for a in asr_agents), str(asr))

    check("agent.invalid_json_config.exit", read_exit("agent/invalid_json_config") == 2, str(read_exit("agent/invalid_json_config")))
    bad_b = (read_text("agent/bad.before.sha256") or "").strip()
    bad_a = (read_text("agent/bad.after.sha256") or "").strip()
    check("agent.invalid_json_config.unchanged", bad_b != "" and bad_b == bad_a, f"before={bad_b} after={bad_a}")

if "mcp" in features:
    check("mcp.session_golden.exit", read_exit("mcp/session_golden") == 0, str(read_exit("mcp/session_golden")))
    mg = read_jsonl("mcp/session_golden.stdout")
    check("mcp.session_golden.count", len(mg) == 5, str(len(mg)))
    if len(mg) == 5:
        init_res = mg[0].get("result", {})
        ping_res = mg[1].get("result")
        tools_res = mg[2].get("result", {}).get("tools", [])
        tool_names = {t.get("name") for t in tools_res}
        check_res = mg[3].get("result", {})
        check_text = check_res.get("content", [{}])[0].get("text", "")
        guard_res = mg[4].get("result", {})
        guard_text = guard_res.get("content", [{}])[0].get("text", "")
        check("mcp.session_golden.initialize", init_res.get("serverInfo", {}).get("name") == "codeint" and "tools" in init_res.get("capabilities", {}), str(init_res))
        check("mcp.session_golden.ping", ping_res == {}, str(ping_res))
        check("mcp.session_golden.tools_list", tool_names == {"check", "diff", "trend", "guard", "impact", "trace", "churn", "coupling"}, str(tool_names))
        check("mcp.session_golden.check_call", check_res.get("isError") is not True and "source_sets" in check_text, check_text[:200])
        check("mcp.session_golden.guard_call", guard_res.get("isError") is not True and "src/sign.ts" in guard_text, guard_text[:200])

    check("mcp.check_ignore_baseline.exit", read_exit("mcp/check_ignore_baseline") == 0, str(read_exit("mcp/check_ignore_baseline")))
    mb = read_jsonl("mcp/check_ignore_baseline.stdout")
    check("mcp.check_ignore_baseline.count", len(mb) == 2, str(len(mb)))
    if len(mb) == 2:
        r_hide = mb[0].get("result", {})
        t_hide = r_hide.get("content", [{}])[0].get("text", "")
        r_ign = mb[1].get("result", {})
        t_ign = r_ign.get("content", [{}])[0].get("text", "")
        check("mcp.check_ignore_baseline.hides", r_hide.get("isError") is not True and "over_budget" not in t_hide and "hidden_findings" in t_hide, t_hide[:300])
        check("mcp.check_ignore_baseline.ignores", "over_budget" in t_ign, t_ign[:300])

    check("mcp.protocol_errors.exit", read_exit("mcp/protocol_errors") == 0, str(read_exit("mcp/protocol_errors")))
    me = read_jsonl("mcp/protocol_errors.stdout")
    check("mcp.protocol_errors.count", len(me) == 4, str(len(me)))
    if len(me) == 4:
        check("mcp.protocol_errors.codes", me[0].get("error", {}).get("code") == -32601 and me[1].get("error", {}).get("code") == -32602 and me[2].get("result", {}).get("isError") is True and me[3].get("error", {}).get("code") == -32700, str(me))

if "update" in features:
    check("update.check_up_to_date.exit", read_exit("update/check_up_to_date") == 0, str(read_exit("update/check_up_to_date")))
    check("update.check_up_to_date.stdout", "is already up to date" in (read_text("update/check_up_to_date.stdout") or ""), read_text("update/check_up_to_date.stdout") or "")

    check("update.check_available.exit", read_exit("update/check_available") == 0, str(read_exit("update/check_available")))
    u_avail = read_text("update/check_available.stdout") or ""
    check("update.check_available.stdout", "Update available:" in u_avail and "v99.0.0" in u_avail, u_avail)

    check("update.local_latest_available.exit", read_exit("update/local_latest_available") == 0, str(read_exit("update/local_latest_available")))
    u_loc = read_text("update/local_latest_available.stdout") or ""
    check("update.local_latest_available.stdout", "Update available:" in u_loc and "v99.0.0" in u_loc, u_loc)

    check("update.local_latest_missing.exit", read_exit("update/local_latest_missing") == 2, str(read_exit("update/local_latest_missing")))
    check("update.local_latest_missing.stderr", "cannot query the latest release" in (read_text("update/local_latest_missing.stderr") or ""), read_text("update/local_latest_missing.stderr") or "")

    check("update.node_modules_refused.exit", read_exit("update/node_modules_refused") == 2, str(read_exit("update/node_modules_refused")))
    check("update.node_modules_refused.stderr", "managed by a package manager" in (read_text("update/node_modules_refused.stderr") or ""), read_text("update/node_modules_refused.stderr") or "")

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
  "$0" doctor "$run_dir" || status=$?
  if [ "$status" -eq 0 ]; then
    "$0" drive "$run_dir" "$feature" || status=$?
  fi
  if [ "$status" -eq 0 ]; then
    "$0" assert "$run_dir" "$feature" || status=$?
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
