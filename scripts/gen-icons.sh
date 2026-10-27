#!/usr/bin/env bash
# Regenerate platform icons from apps/desktop/assets/brand/icon-512.png
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BRAND="$ROOT/apps/desktop/assets/brand"
SRC="$BRAND/icon-512.png"

if [[ ! -f "$SRC" ]]; then
  echo "missing $SRC" >&2
  exit 1
fi

VENV="${TMPDIR:-/tmp}/authorkit-icon-venv"
python3 -m venv "$VENV"
# shellcheck disable=SC1091
source "$VENV/bin/activate"
pip install -q Pillow

python <<PY
from PIL import Image
from pathlib import Path
brand = Path("$BRAND")
img = Image.open(brand / "icon-512.png").convert("RGBA")
for s in (16, 24, 32, 48, 64, 128, 256, 512):
    img.resize((s, s), Image.Resampling.LANCZOS).save(brand / f"icon-{s}.png")
sizes = [256, 128, 64, 48, 32, 16]
frames = [img.resize((s, s), Image.Resampling.LANCZOS) for s in sizes]
frames[0].save(
    brand / "authorkit.ico",
    format="ICO",
    sizes=[(s, s) for s in sizes],
    append_images=frames[1:],
)
print("wrote PNGs + authorkit.ico")
PY

if command -v sips >/dev/null && command -v iconutil >/dev/null; then
  ICONSET="${TMPDIR:-/tmp}/AuthorKit.iconset"
  rm -rf "$ICONSET"
  mkdir -p "$ICONSET"
  for s in 16 32 128 256 512; do
    sips -z "$s" "$s" "$SRC" --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
    d=$((s * 2))
    sips -z "$d" "$d" "$SRC" --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
  done
  iconutil -c icns "$ICONSET" -o "$BRAND/AuthorKit.icns"
  echo "wrote AuthorKit.icns"
else
  echo "skip icns (sips/iconutil not available — run on macOS)"
fi
