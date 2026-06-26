#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$ROOT_DIR/.." && pwd)"
cd "$ROOT_DIR"

if ! command -v cargo >/dev/null 2>&1 && [ -f "$HOME/.cargo/env" ]; then
    # shellcheck disable=SC1091
    . "$HOME/.cargo/env"
fi

cargo build --manifest-path "$REPO_ROOT/Cargo.toml" --release --bin nemotron-engine
swift build -c release

APP_DIR="$ROOT_DIR/.build/release/Nemotron Bubble.app"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"

cp "$ROOT_DIR/.build/release/NemotronBubbleMac" "$APP_DIR/Contents/MacOS/Nemotron Bubble"
cp "$REPO_ROOT/target/release/nemotron-engine" "$APP_DIR/Contents/MacOS/nemotron-engine"
cp "$ROOT_DIR/Resources/Info.plist" "$APP_DIR/Contents/Info.plist"

chmod +x "$APP_DIR/Contents/MacOS/Nemotron Bubble"
chmod +x "$APP_DIR/Contents/MacOS/nemotron-engine"
/usr/bin/codesign --force --sign - "$APP_DIR" >/dev/null

echo "Built: $APP_DIR"
