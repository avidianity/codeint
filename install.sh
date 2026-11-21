#!/bin/sh
set -eu

tmp_dir=""
staged_bin=""

cleanup() {
  if [ -n "${tmp_dir:-}" ]; then
    rm -rf "$tmp_dir"
  fi
  if [ -n "${staged_bin:-}" ]; then
    rm -f "$staged_bin"
  fi
}

fail() {
  printf 'codeint install: %s\n' "$*" >&2
  exit 1
}

detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Darwin) target_os="apple-darwin" ;;
    Linux) target_os="unknown-linux-musl" ;;
    *) fail "unsupported operating system: ${os}" ;;
  esac

  case "$arch" in
    arm64 | aarch64) target_arch="aarch64" ;;
    x86_64 | amd64) target_arch="x86_64" ;;
    *) fail "unsupported architecture: ${arch}" ;;
  esac

  printf '%s-%s\n' "$target_arch" "$target_os"
}

resolve_base_url() {
  if [ -n "${CODEINT_RELEASE_BASE_URL:-}" ]; then
    printf '%s\n' "${CODEINT_RELEASE_BASE_URL%/}"
  elif [ -n "${CODEINT_VERSION:-}" ]; then
    printf 'https://github.com/avidianity/codeint/releases/download/%s\n' "$CODEINT_VERSION"
  else
    printf 'https://github.com/avidianity/codeint/releases/latest/download\n'
  fi
}

resolve_install_dir() {
  if [ -n "${CODEINT_INSTALL_DIR:-}" ]; then
    printf '%s\n' "$CODEINT_INSTALL_DIR"
  elif [ -n "${HOME:-}" ]; then
    printf '%s/.local/bin\n' "$HOME"
  else
    fail "HOME is not set; set CODEINT_INSTALL_DIR to choose an install directory"
  fi
}

fetch_file() {
  base_url="$1"
  filename="$2"
  destination="$3"

  case "$base_url" in
    http://* | https://*)
      url="${base_url}/${filename}"
      if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$url" -o "$destination" || fail "failed to download ${url}"
      elif command -v wget >/dev/null 2>&1; then
        wget -qO "$destination" "$url" || fail "failed to download ${url}"
      else
        fail "curl or wget is required to download ${url}"
      fi
      ;;
    file://*)
      source_path="${base_url#file://}/${filename}"
      [ -f "$source_path" ] || fail "release artifact not found: ${source_path}"
      cp "$source_path" "$destination"
      ;;
    *)
      source_path="${base_url}/${filename}"
      [ -f "$source_path" ] || fail "release artifact not found: ${source_path}"
      cp "$source_path" "$destination"
      ;;
  esac
}

compute_sha256() {
  file_path="$1"

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file_path" | awk '{ print tolower($1) }'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file_path" | awk '{ print tolower($1) }'
  else
    fail "sha256sum or shasum is required to verify release checksums"
  fi
}

expected_sha256() {
  checksums_path="$1"
  archive_filename="$2"

  awk -v file="$archive_filename" '
    { sub(/\r$/, "") }
    $2 == file || $2 == "*" file { print tolower($1); exit }
  ' "$checksums_path"
}

main() {
  trap cleanup EXIT INT TERM

  target="$(detect_target)"
  base_url="$(resolve_base_url)"
  install_dir="$(resolve_install_dir)"
  archive_name="codeint-${target}.tar.gz"

  tmp_dir="$(mktemp -d)"
  checksums_file="${tmp_dir}/SHA256SUMS"
  archive_file="${tmp_dir}/${archive_name}"
  extract_dir="${tmp_dir}/extract"

  fetch_file "$base_url" "SHA256SUMS" "$checksums_file"
  fetch_file "$base_url" "$archive_name" "$archive_file"

  expected_sum="$(expected_sha256 "$checksums_file" "$archive_name")"
  [ -n "$expected_sum" ] || fail "no SHA-256 entry for ${archive_name} in SHA256SUMS"

  actual_sum="$(compute_sha256 "$archive_file")"
  if [ "$actual_sum" != "$expected_sum" ]; then
    printf 'codeint install: SHA-256 checksum mismatch for %s\n  expected: %s\n  actual:   %s\n' \
      "$archive_name" "$expected_sum" "$actual_sum" >&2
    exit 1
  fi

  mkdir -p "$extract_dir"
  tar -xzf "$archive_file" -C "$extract_dir"
  [ -f "${extract_dir}/codeint" ] || fail "archive ${archive_name} does not contain codeint"

  mkdir -p "$install_dir"
  staged_bin="${install_dir}/.codeint.tmp.$$"
  cp "${extract_dir}/codeint" "$staged_bin"
  chmod 0755 "$staged_bin"
  mv -f "$staged_bin" "${install_dir}/codeint"
  staged_bin=""

  printf 'Installed codeint to %s/codeint\n' "$install_dir"
}

main "$@"
