#!/usr/bin/env bash
# Build a double-clickable Glassroom.app for macOS (run this on a Mac).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This packaging script must be run on macOS (found $(uname -s))." >&2
  exit 1
fi

APP_NAME="Glassroom"
BUNDLE_ID="app.glassroom.desktop"
DIST="$ROOT/dist"
APP="$DIST/${APP_NAME}.app"
CONTENTS="$APP/Contents"
MACOS_DIR="$CONTENTS/MacOS"
RES="$CONTENTS/Resources"
ICONSET="$DIST/AppIcon.iconset"
SRC_ICON="$ROOT/assets/AppIcon.png"

echo "==> Building release binary…"
cargo build --release

BIN="$ROOT/target/release/glassroom"
if [[ ! -x "$BIN" ]]; then
  echo "Missing binary at $BIN" >&2
  exit 1
fi

if [[ ! -f "$SRC_ICON" ]]; then
  echo "Missing $SRC_ICON" >&2
  exit 1
fi

echo "==> Assembling ${APP_NAME}.app…"
rm -rf "$APP" "$ICONSET"
mkdir -p "$MACOS_DIR" "$RES" "$ICONSET"

make_icon() {
  local pixels="$1"
  local name="$2"
  sips -z "$pixels" "$pixels" "$SRC_ICON" --out "$ICONSET/$name" >/dev/null
}

make_icon 16   icon_16x16.png
make_icon 32   icon_16x16@2x.png
make_icon 32   icon_32x32.png
make_icon 64   icon_32x32@2x.png
make_icon 128  icon_128x128.png
make_icon 256  icon_128x128@2x.png
make_icon 256  icon_256x256.png
make_icon 512  icon_256x256@2x.png
make_icon 512  icon_512x512.png
make_icon 1024 icon_512x512@2x.png

iconutil -c icns "$ICONSET" -o "$RES/AppIcon.icns"
rm -rf "$ICONSET"

cp "$ROOT/macos/Info.plist" "$CONTENTS/Info.plist"
cp "$BIN" "$MACOS_DIR/Glassroom"
chmod +x "$MACOS_DIR/Glassroom"
printf 'APPL????' > "$CONTENTS/PkgInfo"

echo "==> Built: $APP"
echo "Open with:  open \"$APP\""
echo "Install:    cp -R \"$APP\" /Applications/"
echo "Bundle id:  $BUNDLE_ID"
echo
echo "First launch if Gatekeeper blocks: right-click the app → Open"
echo "Or: xattr -dr com.apple.quarantine \"$APP\""
