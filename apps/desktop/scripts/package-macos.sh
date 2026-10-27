#!/usr/bin/env bash
# Build a minimal AuthorKit.app with the macOS .icns dock icon.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
DESKTOP="$ROOT/apps/desktop"
ICNS="$DESKTOP/assets/brand/AuthorKit.icns"
OUT="${1:-$ROOT/target/AuthorKit.app}"

source "$HOME/.cargo/env" 2>/dev/null || true
cargo build -p authorkit-desktop --release
BIN="$ROOT/target/release/authorkit-desktop"

rm -rf "$OUT"
mkdir -p "$OUT/Contents/MacOS" "$OUT/Contents/Resources"
cp "$BIN" "$OUT/Contents/MacOS/AuthorKit"
cp "$ICNS" "$OUT/Contents/Resources/AuthorKit.icns"

cat > "$OUT/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>fr</string>
  <key>CFBundleExecutable</key><string>AuthorKit</string>
  <key>CFBundleIconFile</key><string>AuthorKit</string>
  <key>CFBundleIdentifier</key><string>app.authorkit.desktop</string>
  <key>CFBundleName</key><string>AuthorKit</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>0.1.0</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

echo "OK → $OUT"
