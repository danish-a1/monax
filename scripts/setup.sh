#!/usr/bin/env bash
# Build-time setup (may use the network; the app never does). Installs only what is missing,
# never runs sudo, never installs a toolchain. --check: report what is missing, change nothing.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHECK=0; [[ "${1:-}" == --check ]] && CHECK=1
RUST_MIN=1.95 NODE_MIN=20
step() { echo "==> $*"; }
die() { echo "ERROR: $*" >&2; exit 1; }
# version_ge A B: true when dotted version A >= B.
version_ge() { [[ "$(printf '%s\n%s\n' "$2" "$1" | sort -V | head -1)" == "$2" ]]; }

step "Checking toolchains"
xcode-select -p >/dev/null 2>&1 || die "Xcode Command Line Tools missing. Run: xcode-select --install"
command -v brew >/dev/null || die "Homebrew missing. Install it from https://brew.sh"
command -v rustc >/dev/null || die "Rust missing. Run: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
RUST="$(rustc --version | awk '{print $2}')"
version_ge "$RUST" "$RUST_MIN" || die "Rust $RUST < $RUST_MIN. Run: rustup update stable"
command -v node >/dev/null || die "Node.js missing. Run: brew install node"
NODE="$(node --version | tr -d v)"
version_ge "$NODE" "$NODE_MIN" || die "Node $NODE < $NODE_MIN. Run: brew upgrade node"
echo "    Xcode CLT ok, Homebrew ok, Rust $RUST, Node $NODE"

# pdfium-binaries chromium/8125 (pdfium 158.0.8125.0); sha256 of the release .tgz.
case "$(uname -m)" in
  arm64) PDFIUM_ARCH=arm64 PDFIUM_SHA=4939bd935ac83ddc09a3a4f038bc43a19cea1660316a3667f26cb07a070e139d ;;
  x86_64) PDFIUM_ARCH=x64 PDFIUM_SHA=8a5193535b80c7215d8932f6647ae5097465b6dba2811602c21e120c2af112b0 ;;
  *) die "unsupported CPU $(uname -m)" ;;
esac
PDFIUM_URL="https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8125/pdfium-mac-$PDFIUM_ARCH.tgz"
PDFIUM_LIB="$ROOT/vendor/pdfium/lib/libpdfium.dylib"

if (( CHECK )); then
  for f in qpdf tesseract; do command -v "$f" >/dev/null && echo "    $f ok" || echo "    $f missing"; done
  [[ -f "$PDFIUM_LIB" ]] && echo "    pdfium ok" || echo "    pdfium missing"
  for f in qpdf tesseract wordcraft-cli; do
    [[ -x "$ROOT/sidecars/$f" ]] && echo "    sidecars/$f ok" || echo "    sidecars/$f missing"
  done
  exit 0
fi

for f in qpdf tesseract; do
  if command -v "$f" >/dev/null; then echo "    $f already installed"
  else step "brew install $f"; brew install "$f"; fi
done

if [[ -f "$PDFIUM_LIB" ]]; then
  echo "    pdfium already in vendor/pdfium"
else
  step "Downloading pdfium from $PDFIUM_URL"
  TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
  curl -fsSL -o "$TMP/pdfium.tgz" "$PDFIUM_URL"
  echo "$PDFIUM_SHA  $TMP/pdfium.tgz" | shasum -a 256 -c - >/dev/null || die "pdfium checksum mismatch"
  mkdir -p "$ROOT/vendor/pdfium"
  tar -xzf "$TMP/pdfium.tgz" -C "$ROOT/vendor/pdfium"
fi

step "npm ci"
(cd "$ROOT" && npm ci)
step "scripts/fetch-sidecars.sh (qpdf, tesseract, WordCraft)"
"$ROOT/scripts/fetch-sidecars.sh"
step "Done. Start Monax with: scripts/run.sh"
