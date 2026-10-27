#!/usr/bin/env bash
# Package a release archive for the current host OS.
# Usage: ./scripts/package-release.sh [version]
# Expects: cargo build -p authorkit-desktop --release already done (or builds it).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="${1:-$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])' 2>/dev/null || true)}"
if [[ -z "${VERSION}" || "${VERSION}" == "null" ]]; then
  VERSION="$(grep -E '^version' Cargo.toml | head -1 | cut -d'"' -f2)"
fi

DIST="$ROOT/dist"
BRAND="$ROOT/apps/desktop/assets/brand"
mkdir -p "$DIST"

if [[ "${SKIP_CARGO_BUILD:-}" == "1" ]]; then
  echo "Skipping cargo build (SKIP_CARGO_BUILD=1); packaging ${VERSION}…"
else
  echo "Building authorkit-desktop ${VERSION}…"
  cargo build -p authorkit-desktop --release
fi

OS="$(uname -s)"
ARCH="$(uname -m)"
case "$ARCH" in
  x86_64|amd64) ARCH_LABEL=x64 ;;
  arm64|aarch64) ARCH_LABEL=arm64 ;;
  *) ARCH_LABEL="$ARCH" ;;
esac

STAGE="$DIST/stage"
rm -rf "$STAGE"
mkdir -p "$STAGE"

package_macos() {
  local app="$STAGE/AuthorKit.app"
  mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
  cp "target/release/authorkit-desktop" "$app/Contents/MacOS/AuthorKit"
  chmod +x "$app/Contents/MacOS/AuthorKit"
  cp "$BRAND/AuthorKit.icns" "$app/Contents/Resources/AuthorKit.icns"
  cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleExecutable</key><string>AuthorKit</string>
  <key>CFBundleIconFile</key><string>AuthorKit</string>
  <key>CFBundleIdentifier</key><string>fr.supralab.authorkit</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>AuthorKit</string>
  <key>CFBundleDisplayName</key><string>AuthorKit</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key>
  <string>Copyright © 2026 Julien Moulin / SupraLab (https://supralab.fr/en)</string>
</dict>
</plist>
PLIST
  local out="$DIST/AuthorKit-${VERSION}-macos-${ARCH_LABEL}.zip"
  rm -f "$out"
  (cd "$STAGE" && zip -qry "$out" AuthorKit.app)
  echo "Wrote $out"
}

package_linux() {
  local name="AuthorKit-${VERSION}-linux-${ARCH_LABEL}"
  local dir="$STAGE/$name"
  mkdir -p "$dir/bin" "$dir/share/icons/hicolor/512x512/apps" "$dir/share/applications"
  cp "target/release/authorkit-desktop" "$dir/bin/authorkit"
  chmod +x "$dir/bin/authorkit"
  cp "$BRAND/icon-512.png" "$dir/share/icons/hicolor/512x512/apps/authorkit.png"
  mkdir -p "$dir/share/icons/hicolor/256x256/apps"
  cp "$BRAND/icon-256.png" "$dir/share/icons/hicolor/256x256/apps/authorkit.png"
  cat > "$dir/share/applications/authorkit.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=AuthorKit
Comment=Local-first writing studio for storytellers
Exec=authorkit
Icon=authorkit
Categories=Office;TextEditor;
Terminal=false
StartupWMClass=AuthorKit
DESKTOP
  cat > "$dir/README.txt" <<EOF
AuthorKit ${VERSION}
https://supralab.fr/en

Run: ./bin/authorkit
Optional: copy share/applications and share/icons into ~/.local/
EOF
  local out="$DIST/${name}.tar.gz"
  rm -f "$out"
  (cd "$STAGE" && tar -czf "$out" "$name")
  echo "Wrote $out"
}

package_windows() {
  local name="AuthorKit-${VERSION}-windows-${ARCH_LABEL}"
  local dir="$STAGE/$name"
  mkdir -p "$dir"
  local exe="target/release/authorkit-desktop.exe"
  if [[ ! -f "$exe" ]]; then
    exe="target/release/authorkit-desktop"
  fi
  cp "$exe" "$dir/AuthorKit.exe"
  cp "$BRAND/authorkit.ico" "$dir/authorkit.ico"
  cat > "$dir/README.txt" <<EOF
AuthorKit ${VERSION}
https://supralab.fr/en

Double-click AuthorKit.exe to launch.
EOF
  local out="$DIST/${name}.zip"
  rm -f "$out"
  if command -v zip >/dev/null; then
    (cd "$STAGE" && zip -qry "$out" "$name")
  else
    (cd "$STAGE" && powershell -Command "Compress-Archive -Path '$name' -DestinationPath '$out'")
  fi
  echo "Wrote $out"
}

case "$OS" in
  Darwin) package_macos ;;
  Linux) package_linux ;;
  MINGW*|MSYS*|CYGWIN*|Windows_NT) package_windows ;;
  *)
    # GitHub Actions Windows runners report MINGW via bash; also handle via OSTYPE.
    if [[ "${OSTYPE:-}" == msys* || "${OSTYPE:-}" == cygwin* ]]; then
      package_windows
    else
      echo "Unsupported OS: $OS" >&2
      exit 1
    fi
    ;;
esac

ls -la "$DIST"
