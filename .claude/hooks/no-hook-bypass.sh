#!/usr/bin/env bash
set -euo pipefail

input="$(cat)"
if ! command -v jq >/dev/null 2>&1; then
  echo "Blocked: the hook bypass guard needs jq. Install jq and try again." >&2
  exit 2
fi
command_text="$(printf '%s' "$input" | jq -r '.tool_input.command // ""')"
file_path="$(printf '%s' "$input" | jq -r '.tool_input.file_path // .tool_input.notebook_path // ""')"

refuse() {
  echo "Blocked: this $1 skips Git hooks. Fix the hook failure and commit again; see docs/checks/no-hook-bypass.md." >&2
  exit 2
}

case "$file_path" in
  *lefthook-local* | *.lefthook-local* | */.git/hooks/* | .git/hooks/*) refuse "file change" ;;
esac

[ -n "$command_text" ] || exit 0

normalized="$(printf '%s' "$command_text" | tr '\t\r\n' '   ')"
without_allowed="$(printf '%s' "$normalized" | sed -E 's/LEFTHOOK_(VERBOSE|QUIET)=//g')"

patterns=(
  '(^|[[:space:]])--no-v[a-z]*'
  'git([[:space:]]+[^|;&]*)?[[:space:]]commit([[:space:]]+[^|;&]*)?[[:space:]]-[a-zA-Z]*n[a-zA-Z]*([[:space:]]|$)'
  'core\.hookspath'
  'GIT_CONFIG[A-Z_]*='
  'LEFTHOOK[A-Z_]*='
  'lefthook(-local|[[:space:]]+uninstall)'
  '\.git/hooks'
  'hooks/(pre-commit|commit-msg|prepare-commit-msg|pre-push|pre-merge-commit|post-commit|pre-rebase)'
)
for pattern in "${patterns[@]}"; do
  if printf '%s' "$without_allowed" | grep -Eiq -- "$pattern"; then
    refuse "command"
  fi
done
exit 0
