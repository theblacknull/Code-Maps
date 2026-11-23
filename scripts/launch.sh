#!/usr/bin/env bash
# Launch ua-viewer for the current platform.
# Order: bundled binary -> download from this version's GitHub Release -> build from source.
# Usage: launch.sh <path/to/knowledge-graph.json>
set -euo pipefail

GRAPH="${1:-.ua/knowledge-graph.json}"
ROOT="${CLAUDE_PLUGIN_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
REPO="theblacknull/Code-Maps"

if [ ! -f "$GRAPH" ]; then
  echo "Không thấy $GRAPH. Chạy /understand trước." >&2
  exit 1
fi

# Khóa nền tảng: windows-x64, macos-arm64, macos-x64, linux-x64
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) OS=windows; EXT=.exe ;;
  Darwin) OS=macos; EXT= ;;
  Linux) OS=linux; EXT= ;;
  *) echo "Hệ điều hành chưa được hỗ trợ: $(uname -s)" >&2; exit 1 ;;
esac
case "$(uname -m)" in
  x86_64|amd64) ARCH=x64 ;;
  arm64|aarch64) ARCH=arm64 ;;
  *) echo "Kiến trúc chưa được hỗ trợ: $(uname -m)" >&2; exit 1 ;;
esac
KEY="$OS-$ARCH"
[ "$OS" = windows ] && KEY="windows-x64"   # chỉ có bản x64 cho Windows

VERSION=$(sed -n 's/.*"version": *"\([^"]*\)".*/\1/p' "$ROOT/.claude-plugin/plugin.json" | head -1)
BIN="$ROOT/bin/$KEY/ua-viewer$EXT"

if [ ! -x "$BIN" ] && [ ! -f "$BIN" ]; then
  mkdir -p "$(dirname "$BIN")"
  URL="https://github.com/$REPO/releases/download/v$VERSION/ua-viewer-$KEY$EXT"
  if command -v curl >/dev/null 2>&1 && curl -fsSL "$URL" -o "$BIN.tmp" 2>/dev/null; then
    mv "$BIN.tmp" "$BIN" && chmod +x "$BIN"
    echo "Đã tải ua-viewer $VERSION cho $KEY." >&2
  else
    rm -f "$BIN.tmp"
    if ! command -v cargo >/dev/null 2>&1; then
      echo "Không tải được bản dựng sẵn và không có cargo. Cài Rust (https://rustup.rs) rồi chạy lại." >&2
      exit 1
    fi
    echo "Đang build từ mã nguồn (lần đầu mất vài phút)..." >&2
    (cd "$ROOT" && cargo build --release)
    cp "$ROOT/target/release/ua-viewer$EXT" "$BIN"
  fi
fi

exec "$BIN" "$GRAPH"
