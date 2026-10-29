#!/usr/bin/env bash
# Dev/build tool: sidecars/licenses/THIRD-PARTY.txt for every crate and npm package compiled
# into the app (normal, non-dev, non-build closure of `monax` for macOS). Run by fetch-sidecars.sh.
# Fails on a GPL/AGPL/LGPL-only license. Run with --self-check to test the gate alone.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PY="$ROOT/scripts/third_party.py"
[[ "${1:-}" == --self-check ]] && exec python3 -I "$PY" --self-check
OUT="${OUT:-$ROOT/sidecars}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
cd "$ROOT"
mkdir -p "$OUT/licenses"
cargo metadata --locked --format-version 1 >"$TMP/meta.json"
# ponytail: aarch64 only, the app is not built universal; add x86_64-apple-darwin if that changes.
cargo tree --locked -p monax -e normal --target aarch64-apple-darwin --prefix none >"$TMP/tree.txt"
npm ls --omit=dev --all --long --json >"$TMP/npm.json"
python3 -I "$PY" "$OUT/licenses/THIRD-PARTY.txt" "$TMP/meta.json" "$TMP/tree.txt" "$TMP/npm.json"
