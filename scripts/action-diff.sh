#!/usr/bin/env bash
set -euo pipefail
BASE="$INPUT_BASE_REF"
if [ -z "$BASE" ]; then
  if [ -n "$GITHUB_BASE_REF" ]; then
    BASE="origin/$GITHUB_BASE_REF"
  else
    BASE="HEAD~1"
  fi
fi
WORKSPACE="$PWD"
MD_FILE="codeint-report.md"
SARIF_FILE="codeint.sarif"
cd "$INPUT_PATH"
set +e
codeint diff "$BASE" --format markdown --output "$WORKSPACE/$MD_FILE"
EXIT_CODE=$?
set -e
echo "exit-code=$EXIT_CODE" >> "$GITHUB_OUTPUT"
echo "markdown-file=$MD_FILE" >> "$GITHUB_OUTPUT"
if [ "$INPUT_UPLOAD_SARIF" = "true" ]; then
  set +e
  codeint diff "$BASE" --format sarif --output "$WORKSPACE/$SARIF_FILE"
  SARIF_EXIT_CODE=$?
  set -e
  echo "sarif-exit-code=$SARIF_EXIT_CODE" >> "$GITHUB_OUTPUT"
  if [ "$SARIF_EXIT_CODE" != "0" ] && [ "$SARIF_EXIT_CODE" != "1" ]; then
    printf 'codeint action: SARIF pass exited with unexpected code %s\n' "$SARIF_EXIT_CODE" >&2
    exit "$SARIF_EXIT_CODE"
  fi
  echo "sarif-file=$SARIF_FILE" >> "$GITHUB_OUTPUT"
fi
