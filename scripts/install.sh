#!/usr/bin/env bash
set -euo pipefail

REPO="radjathaher/penpot-cli"
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

case "$OS" in
  darwin) OS="darwin" ;;
  *) echo "unsupported OS: $OS (only macOS arm64 supported)"; exit 1 ;;
esac

case "$ARCH" in
  arm64|aarch64) ARCH="aarch64" ;;
  *) echo "unsupported arch: $ARCH (only arm64 supported)"; exit 1 ;;
esac

VERSION="${PENPOT_CLI_VERSION:-latest}"

if [[ "$VERSION" == "latest" ]]; then
  api_url="https://api.github.com/repos/${REPO}/releases/latest"
else
  api_url="https://api.github.com/repos/${REPO}/releases/tags/${VERSION}"
fi

API_URL="$api_url" OS_NAME="$OS" ARCH_NAME="$ARCH" asset_url=$(python - <<'PY'
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

TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

curl -fsSL "$asset_url" -o "$TMP_DIR/penpot-cli.tar.gz"
tar -xzf "$TMP_DIR/penpot-cli.tar.gz" -C "$TMP_DIR"

BIN_DIR="${BIN_DIR:-$HOME/.local/bin}"
mkdir -p "$BIN_DIR"
install -m 755 "$TMP_DIR/penpot" "$BIN_DIR/penpot"

echo "installed: $BIN_DIR/penpot"
