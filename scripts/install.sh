#!/usr/bin/env bash
set -euo pipefail

REPO="radjathaher/penpot-cli"
BIN="penpot"
VERSION="${PENPOT_CLI_VERSION:-latest}"

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

case "$OS" in
  darwin) OS="darwin" ;;
  linux) OS="linux" ;;
  *) echo "unsupported OS: $OS (supported: macOS, Linux)" >&2; exit 1 ;;
esac

case "$ARCH" in
  arm64|aarch64) ARCH="aarch64" ;;
  x86_64|amd64) ARCH="x86_64" ;;
  *) echo "unsupported arch: $ARCH (supported: arm64, x86_64)" >&2; exit 1 ;;
esac

if [[ "$VERSION" == "latest" ]]; then
  api_url="https://api.github.com/repos/${REPO}/releases/latest"
else
  api_url="https://api.github.com/repos/${REPO}/releases/tags/${VERSION}"
fi

PYTHON_BIN="${PYTHON_BIN:-}"
if [[ -z "$PYTHON_BIN" ]]; then
  if command -v python3 >/dev/null 2>&1; then
    PYTHON_BIN="python3"
  elif command -v python >/dev/null 2>&1; then
    PYTHON_BIN="python"
  else
    echo "python3 is required to install ${BIN}" >&2
    exit 1
  fi
fi

asset_url=$(API_URL="$api_url" OS_NAME="$OS" ARCH_NAME="$ARCH" "$PYTHON_BIN" - <<'PY'
import json
import os
import sys
import urllib.request

url = os.environ["API_URL"]
os_name = os.environ["OS_NAME"]
arch = os.environ["ARCH_NAME"]

with urllib.request.urlopen(url) as f:
    data = json.load(f)

assets = data.get("assets", [])
want_suffix = f"{os_name}-{arch}.tar.gz"
for a in assets:
    name = a.get("name", "")
    if name.endswith(want_suffix):
        print(a.get("browser_download_url"))
        sys.exit(0)

print("")
PY
)

if [[ -z "$asset_url" ]]; then
  echo "no release asset for ${OS}-${ARCH}" >&2
  exit 1
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

curl -fsSL "$asset_url" -o "$tmp_dir/${BIN}.tar.gz"
tar -xzf "$tmp_dir/${BIN}.tar.gz" -C "$tmp_dir"

install_dir="${BIN_DIR:-}"
if [[ -z "$install_dir" ]]; then
  if [[ -w "/usr/local/bin" ]]; then
    install_dir="/usr/local/bin"
  else
    install_dir="$HOME/.local/bin"
  fi
fi

mkdir -p "$install_dir"
install -m 755 "$tmp_dir/${BIN}" "$install_dir/${BIN}"

echo "installed: $install_dir/${BIN}"
