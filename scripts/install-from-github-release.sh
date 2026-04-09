#!/usr/bin/env bash
set -euo pipefail

REPO="pashpashpash/capcut-cli"
RELEASE_BASE_URL="${CAPCUT_CLI_RELEASE_BASE_URL:-https://github.com/${REPO}/releases/latest/download}"

usage() {
  echo "usage: $0 [--bin-dir <dir>]" >&2
  exit 1
}

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "${os}:${arch}" in
    Darwin:arm64) echo "aarch64-apple-darwin" ;;
    Darwin:x86_64) echo "x86_64-apple-darwin" ;;
    Linux:x86_64) echo "x86_64-unknown-linux-gnu" ;;
    Linux:aarch64) echo "aarch64-unknown-linux-gnu" ;;
    *)
      echo "unsupported platform: ${os}/${arch}" >&2
      exit 1
      ;;
  esac
}

default_bin_dir() {
  for dir in /opt/homebrew/bin /usr/local/bin; do
    if [[ -d "${dir}" && -w "${dir}" ]]; then
      echo "${dir}"
      return
    fi
  done

  echo "${HOME}/.local/bin"
}

BIN_DIR=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --bin-dir)
      [[ $# -ge 2 ]] || usage
      BIN_DIR="$2"
      shift 2
      ;;
    *)
      usage
      ;;
  esac
done

TARGET="$(detect_target)"
ASSET="capcut-cli-${TARGET}.tar.gz"
BIN_DIR="${BIN_DIR:-$(default_bin_dir)}"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

mkdir -p "${BIN_DIR}"
curl -fsSL \
  "${RELEASE_BASE_URL%/}/${ASSET}" \
  -o "${TMP_DIR}/${ASSET}"
tar -xzf "${TMP_DIR}/${ASSET}" -C "${TMP_DIR}"
install -m 755 "${TMP_DIR}/capcut-cli" "${BIN_DIR}/capcut-cli"

echo "installed ${BIN_DIR}/capcut-cli"
if ! command -v capcut-cli >/dev/null 2>&1 || [[ "$(command -v capcut-cli)" != "${BIN_DIR}/capcut-cli" ]]; then
  echo "ensure ${BIN_DIR} is on PATH to invoke capcut-cli directly"
fi
