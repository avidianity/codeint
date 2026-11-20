#!/usr/bin/env bash
set -euo pipefail
input="$(cat)"
pattern='git[^|;&]*--no-verify|git commit[^|;&]* -n( |"|$)|git[^|;&]*core\.hooksPath|LEFTHOOK=0|LEFTHOOK_EXCLUDE='
if printf '%s' "$input" | grep -Eq -- "$pattern"; then
  echo "Blocked: this command skips Git hooks. Fix the hook failure and commit again; see docs/checks/no-hook-bypass.md." >&2
  exit 2
fi
exit 0
