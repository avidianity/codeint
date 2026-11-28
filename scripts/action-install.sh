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
INSTALLER_FILE="$(mktemp)"
trap 'rm -f "$INSTALLER_FILE"' EXIT
if [ "$VERSION" = "latest" ]; then
  URL="https://github.com/avidianity/codeint/releases/latest/download/install.sh"
  unset CODEINT_VERSION
  curl -fsSL -o "$INSTALLER_FILE" "$URL"
  bash "$INSTALLER_FILE"
else
  URL="https://github.com/avidianity/codeint/releases/download/${VERSION}/install.sh"
  curl -fsSL -o "$INSTALLER_FILE" "$URL"
  CODEINT_VERSION="$VERSION" bash "$INSTALLER_FILE"
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
