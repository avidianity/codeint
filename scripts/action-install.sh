#!/usr/bin/env bash
set -euo pipefail
VERSION="${INPUT_VERSION:-latest}"
if [ -z "$VERSION" ]; then
  VERSION="latest"
fi
if [ -n "${CODEINT_INSTALL_DIR:-}" ]; then
  INSTALL_DIR="$CODEINT_INSTALL_DIR"
elif [ -n "${HOME:-}" ]; then
  INSTALL_DIR="$HOME/.local/bin"
else
  printf 'codeint action: neither CODEINT_INSTALL_DIR nor HOME is set\n' >&2
  exit 1
fi
if [ "$VERSION" = "latest" ]; then
  URL="https://github.com/avidianity/codeint/releases/latest/download/install.sh"
  unset CODEINT_VERSION
  curl -fsSL "$URL" | bash
else
  URL="https://github.com/avidianity/codeint/releases/download/${VERSION}/install.sh"
  curl -fsSL "$URL" | CODEINT_VERSION="$VERSION" bash
fi
if [ "$VERSION" != "latest" ]; then
  EXPECTED_VERSION="${VERSION#v}"
  INSTALLED_VERSION="$("$INSTALL_DIR/codeint" --version | tr -d '\r')"
  if [ "$INSTALLED_VERSION" != "codeint $EXPECTED_VERSION" ]; then
    printf 'codeint action: expected installed version codeint %s for tag %s, got %s\n' \
      "$EXPECTED_VERSION" "$VERSION" "$INSTALLED_VERSION" >&2
    exit 1
  fi
fi
echo "$INSTALL_DIR" >> "$GITHUB_PATH"
