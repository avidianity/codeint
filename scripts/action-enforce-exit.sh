#!/usr/bin/env bash
set -euo pipefail
fail_with_code() {
  local label="$1"
  local code="$2"
  if [[ "$code" =~ ^[1-9][0-9]*$ ]] && [ "$code" -le 255 ]; then
    printf 'codeint action: %s failed with exit code %s\n' "$label" "$code" >&2
    exit "$code"
  fi
  printf 'codeint action: invalid or missing %s exit code: %s\n' "$label" "$code" >&2
  exit 1
}
DIFF_CODE="${DIFF_EXIT_CODE:-}"
case "$DIFF_CODE" in
  0) ;;
  1)
    if [ "${FAIL_ON_REGRESSION:-true}" != "false" ]; then
      exit 1
    fi
    ;;
  *)
    fail_with_code "codeint diff" "$DIFF_CODE"
    ;;
esac
if [ "${INPUT_UPLOAD_SARIF:-false}" = "true" ]; then
  SARIF_CODE="${SARIF_EXIT_CODE:-}"
  case "$SARIF_CODE" in
    0|1) ;;
    *)
      fail_with_code "SARIF pass" "$SARIF_CODE"
      ;;
  esac
fi
