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
  set -e
  echo "sarif-file=$SARIF_FILE" >> "$GITHUB_OUTPUT"
fi
