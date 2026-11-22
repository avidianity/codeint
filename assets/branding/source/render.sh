#!/bin/sh
set -eu
cd "$(dirname "$0")"
chrome="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
shot() {
  "$chrome" --headless --disable-gpu --hide-scrollbars --force-device-scale-factor=1 \
    --default-background-color=00000000 --window-size="$2" --virtual-time-budget=10000 \
    --screenshot="$work/$3" "file://$PWD/$1" >"$work/chrome.log" 2>&1 || true
  if [ ! -s "$work/$3" ]; then
    cat "$work/chrome.log" >&2
    echo "render.sh: Chrome did not write $3" >&2
    exit 1
  fi
}
shot banner.html 1600,700 banner.png
shot banner.html?social 1600,800 social-preview.png
shot lockup.html 880,260 lockup-dark.png
shot lockup.html?theme=light 880,260 lockup-light.png
cwebp -quiet -q 90 "$work/banner.png" -o ../banner.webp
cp "$work/social-preview.png" "$work/lockup-dark.png" "$work/lockup-light.png" ..
