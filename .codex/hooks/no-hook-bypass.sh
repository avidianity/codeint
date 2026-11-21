#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
guard="$script_dir/../../.claude/hooks/no-hook-bypass.sh"
input="$(cat)"

if ! command -v jq >/dev/null 2>&1; then
  echo "Blocked: the hook bypass guard needs jq. Install jq and try again." >&2
  exit 2
fi

tool_name="$(printf '%s' "$input" | jq -r '.tool_name // ""')"
if [ "$tool_name" = "apply_patch" ]; then
  patch_text="$(printf '%s' "$input" | jq -r '.tool_input.command // .tool_input.patch // .tool_input.input // ""')"
  while IFS= read -r line; do
    line="${line%$'\r'}"
    case "$line" in
      "*** Add File:"* | "*** Update File:"* | "*** Delete File:"* | "*** Move to:"*)
        target_path="${line#*:}"
        target_path="${target_path#"${target_path%%[![:space:]]*}"}"
        target_path="${target_path%"${target_path##*[![:space:]]}"}"
        payload="$(jq -nc --arg path "$target_path" '{ tool_input: { file_path: $path } }')"
        printf '%s' "$payload" | bash "$guard"
        ;;
    esac
  done <<< "$patch_text"
  exit 0
fi

printf '%s' "$input" | bash "$guard"
