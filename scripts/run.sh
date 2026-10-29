#!/usr/bin/env bash
# Runs Monax from source. Default: build the release binary once (no .app, no DMG), then run it.
# --dev: `npm run tauri dev` (hot reload). Both use sidecars/ and vendor/pdfium from setup.sh.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export MONAX_BIN_DIR="$ROOT/sidecars"
export MONAX_LIBPDFIUM="$ROOT/vendor/pdfium/lib/libpdfium.dylib"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
BIN="$CARGO_TARGET_DIR/release/monax"

for f in "$MONAX_BIN_DIR/qpdf" "$MONAX_BIN_DIR/tesseract" "$MONAX_BIN_DIR/wordcraft-cli" "$MONAX_LIBPDFIUM"; do
  [[ -f "$f" ]] || { echo "ERROR: $f is missing. Run scripts/setup.sh first." >&2; exit 1; }
done
[[ -d "$ROOT/node_modules" ]] || { echo "ERROR: node_modules is missing. Run scripts/setup.sh first." >&2; exit 1; }

cd "$ROOT"
if [[ "${1:-}" == --dev ]]; then
  exec npm run tauri dev
fi
# ponytail: builds only when the binary is absent; after a git pull run `npm run tauri build -- --no-bundle`
# (or delete the binary) to rebuild.
if [[ ! -x "$BIN" ]]; then
  echo "==> Building Monax (first run, takes several minutes)"
  npm run tauri build -- --no-bundle
fi
exec "$BIN"
