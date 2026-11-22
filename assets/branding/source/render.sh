#!/bin/sh
set -eu
cd "$(dirname "$0")"
chrome="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
out="${1:-..}"
shot() {
  "$chrome" --headless --disable-gpu --hide-scrollbars --force-device-scale-factor="$4" \
    --default-background-color=00000000 --window-size="$2" --virtual-time-budget=10000 \
    --screenshot="$out/$3" "file://$PWD/$1" >/dev/null 2>&1
}
shot banner.html 1600,700 banner.png 1
shot banner.html?social 1600,800 social-preview.png 1
shot lockup.html 880,260 lockup-dark.png 1
shot lockup.html?theme=light 880,260 lockup-light.png 1
cwebp -quiet -q 90 "$out/banner.png" -o "$out/banner.webp"
