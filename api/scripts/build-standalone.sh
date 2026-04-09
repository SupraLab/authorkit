#!/usr/bin/env bash
# Build AuthorKit API standalone bundle (PyInstaller one-dir + zip). Same script as CI.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
API_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$API_ROOT"

detect_triplet() {
  local os
  case "$(uname -s)" in
    Darwin) os=darwin ;;
    Linux) os=linux ;;
    MINGW* | MSYS* | CYGWIN*) os=win ;;
    *)
      echo "Unsupported OS: $(uname -s)" >&2
      exit 1
      ;;
  esac
  local arch
  arch="$(uname -m)"
  case "$arch" in
    x86_64 | amd64)
      if [[ "$os" == "win" ]]; then
        echo "win-amd64"
      else
        echo "${os}-x64"
      fi
      ;;
    arm64 | aarch64) echo "${os}-arm64" ;;
    *) echo "${os}-${arch}" ;;
  esac
}

TRIPLET="${AUTHOR_KIT_PLATFORM:-$(detect_triplet)}"

if [[ -n "${PYTHON:-}" ]]; then
  :
elif command -v python3 >/dev/null 2>&1; then
  PYTHON="python3"
else
  PYTHON="python"
fi

if [[ ! -d "${API_ROOT}/.venv" ]]; then
  "$PYTHON" -m venv "${API_ROOT}/.venv"
fi
# shellcheck source=/dev/null
if [[ -f "${API_ROOT}/.venv/bin/activate" ]]; then
  source "${API_ROOT}/.venv/bin/activate"
elif [[ -f "${API_ROOT}/.venv/Scripts/activate" ]]; then
  source "${API_ROOT}/.venv/Scripts/activate"
else
  echo "Could not find venv activate script" >&2
  exit 1
fi
python -m pip install -U pip -q
python -m pip install -e ".[dev]" -q

rm -rf "${API_ROOT}/build" "${API_ROOT}/dist"
python -m PyInstaller --noconfirm "${API_ROOT}/author-kit-api.spec"

VERSION="$(python -c "from importlib.metadata import version; print(version('author-kit-api'))")"
RELEASE_DIR="${API_ROOT}/dist/release"
mkdir -p "${RELEASE_DIR}"
ZIP_NAME="author-kit-api-${VERSION}-${TRIPLET}.zip"
ARCHIVE_BASE="${ZIP_NAME%.zip}"
rm -f "${RELEASE_DIR}/${ZIP_NAME}"
# Use cwd (we are in API_ROOT): avoids Git-Bash /d/... paths breaking pathlib on Windows CI.
python -c "import shutil; from pathlib import Path; r = Path.cwd(); shutil.make_archive(str(r / 'dist' / 'release' / '${ARCHIVE_BASE}'), 'zip', root_dir=r / 'dist', base_dir='author-kit-api')"

echo "Built: ${RELEASE_DIR}/${ZIP_NAME}"
