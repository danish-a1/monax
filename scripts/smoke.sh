#!/usr/bin/env bash
# Release smoke test: the built DMG's app, run without Homebrew on PATH, driven
# over its embedded MCP server. Usage: scripts/smoke.sh [path/to.dmg]
# Build first: scripts/fetch-sidecars.sh && npm run tauri build
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FIX="$ROOT/crates/monax-core/tests/fixtures"
DMG="${1:-$(ls -t "$ROOT"/target/release/bundle/dmg/*.dmg 2>/dev/null | head -1)}"
CLEAN_PATH=/usr/bin:/bin:/usr/sbin:/sbin
STEP="setup"
TMP="$(mktemp -d)"
MNT="$TMP/mnt"
PID=""

cleanup() {
  [[ -n "$PID" ]] && { kill "$PID" 2>/dev/null || true; wait "$PID" 2>/dev/null || true; }
  [[ -d "$MNT" ]] && hdiutil detach -quiet "$MNT" 2>/dev/null || true
  rm -rf "$TMP"
}
trap cleanup EXIT
trap 'echo "FAIL [$STEP] line $LINENO" >&2' ERR
fail() { echo "FAIL [$STEP]: $*" >&2; exit 1; }
step() { STEP="$*"; echo "== $STEP"; }
# Runs a command with no Homebrew on PATH and no Monax dev overrides.
clean() { env -i HOME="$HOME" TMPDIR="${TMPDIR:-/tmp}" PATH="$CLEAN_PATH" "$@"; }

step "mount DMG"
[[ -f "$DMG" ]] || fail "no DMG (build with npm run tauri build)"
mkdir "$MNT"
hdiutil attach -readonly -nobrowse -quiet -mountpoint "$MNT" "$DMG"
APP_SRC="$(ls -d "$MNT"/*.app | head -1)"
cp -R "$APP_SRC" "$TMP/"
hdiutil detach -quiet "$MNT" && rmdir "$MNT"
APP="$TMP/$(basename "$APP_SRC")"
EXE="$APP/Contents/MacOS/$(plutil -extract CFBundleExecutable raw -o - "$APP/Contents/Info.plist")"
SC="$APP/Contents/Resources/sidecars"
QPDF="$SC/qpdf"

step "bundle layout"
for f in "$QPDF" "$SC/tesseract" "$SC/wordcraft-cli" "$SC/tessdata/eng.traineddata" \
  "$APP/Contents/Frameworks/libpdfium.dylib"; do
  [[ -f "$f" ]] || fail "missing ${f#"$APP/"}"
done
for l in qpdf tesseract tessdata leptonica pdfium wordcraft; do
  [[ -n "$(ls "$APP/Contents/Resources/licenses/$l" 2>/dev/null)" ]] || fail "no license for $l"
done

TP="$APP/Contents/Resources/licenses/THIRD-PARTY.txt"
[[ -s "$TP" ]] || fail "no THIRD-PARTY.txt"
for t in jpeg-encoder "Independent JPEG Group"; do
  grep -q "$t" "$TP" || fail "THIRD-PARTY.txt lacks $t"
done

step "sidecars run without Homebrew"
QV="$(clean "$QPDF" --version | head -1)" || fail "qpdf --version"
TV="$(clean "$SC/tesseract" --version 2>&1 | head -1)" || fail "tesseract --version"
WV="$(clean "$SC/wordcraft-cli" --version | head -1)" || fail "wordcraft-cli --version"
echo "   $QV / $TV / $WV"

step "Mach-O references and signatures"
while IFS= read -r -d '' f; do
  file -b "$f" | grep -q 'Mach-O' || continue
  if otool -L "$f" | tail -n +2 | grep -E '/opt/homebrew|/usr/local|@rpath/'; then
    fail "${f#"$APP/"} references a non-bundled library"
  fi
  codesign --verify --strict "$f" || fail "bad signature: ${f#"$APP/"}"
done < <(find "$APP/Contents" -type f -print0)
codesign --verify --deep --strict "$APP" || fail "bad app signature"

step "launch app"
CFG="$TMP/config"
env -i HOME="$HOME" TMPDIR="${TMPDIR:-/tmp}" PATH="$CLEAN_PATH" MONAX_CONFIG_DIR="$CFG" \
  "$EXE" >"$TMP/app.log" 2>&1 &
PID=$!
for _ in $(seq 150); do
  [[ -f "$CFG/mcp.json" ]] && break
  kill -0 "$PID" 2>/dev/null || { cat "$TMP/app.log" >&2; fail "app exited"; }
  sleep 0.2
done
[[ -f "$CFG/mcp.json" ]] || fail "no mcp.json after 30s"
URL="$(plutil -extract url raw -o - "$CFG/mcp.json")"
# Token goes into a 0600 header file, never onto argv or stdout.
(umask 077; printf 'Authorization: Bearer %s\n' \
  "$(plutil -extract token raw -o - "$CFG/mcp.json")" >"$TMP/auth")

# rpc METHOD PARAMS_JSON: POSTs JSON-RPC, leaves the response in $TMP/resp.json.
rpc() {
  curl -sS --fail-with-body -m 30 -o "$TMP/resp.json" -H @"$TMP/auth" \
    -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
    --data-binary "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$1\",\"params\":$2}" "$URL" \
    || fail "$1: HTTP error $(cat "$TMP/resp.json" 2>/dev/null)"
}
get() { plutil -extract "$1" raw -o - "$TMP/resp.json"; }
# call TOOL ARGS_JSON: tools/call that must not be an error.
call() {
  rpc tools/call "{\"name\":\"$1\",\"arguments\":$2}"
  [[ "$(get result.isError 2>/dev/null)" != true ]] || fail "$1: $(get result.content.0.text)"
}

step "MCP initialize"
rpc initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"smoke","version":"1"}}'
get result.serverInfo.name >/dev/null || fail "no serverInfo"

step "pdf_capabilities"
call pdf_capabilities '{}'
[[ "$(get result.structuredContent.qpdf)" == "$QV" ]] || fail "qpdf not found in bundle"
[[ "$(get result.structuredContent.tesseract)" == "$TV" ]] || fail "tesseract not found in bundle"
[[ "$(get result.structuredContent.ocr_languages.0)" == eng ]] || fail "eng not in bundled tessdata"
[[ "$(get result.structuredContent.wordcraft)" == "$WV" ]] || fail "wordcraft-cli not found in bundle"

W="$TMP/work"
mkdir "$W"
cp "$FIX/three-pages.pdf" "$FIX/outlines.pdf" "$FIX/image.pdf" "$FIX/annots.pdf" \
  "$FIX/encrypted.pdf" "$FIX/owner-only.pdf" "$FIX/invoice-scan.pdf" "$FIX/form.pdf" \
  "$FIX/signature.png" "$FIX/office/hello.docx" "$W/"
P="$W/three-pages.pdf" O="$W/outlines.pdf" IMG="$W/image.pdf" AN="$W/annots.pdf"
ENC="$W/encrypted.pdf" OWN="$W/owner-only.pdf" INV="$W/invoice-scan.pdf" FORM="$W/form.pdf"
SIG="$W/signature.png"
RECT='{"left":10,"bottom":10,"right":100,"top":100}'

# One row per operation that exists: tool | arguments | output kind (pdf/png/jpeg/json/fields).
# Later tasks add their rows here.
ROWS=(
  "pdf_extract|{\"input\":\"$P\",\"pages\":[0,2]}|pdf"
  "pdf_delete|{\"input\":\"$P\",\"pages\":[1]}|pdf"
  "pdf_rotate|{\"input\":\"$P\",\"pages\":[0],\"degrees\":90}|pdf"
  "pdf_reorder|{\"input\":\"$P\",\"order\":[2,1,0]}|pdf"
  "pdf_organize|{\"input\":\"$P\",\"plan\":[{\"page\":2,\"rotate\":90}]}|pdf"
  "pdf_merge|{\"inputs\":[\"$P\",\"$O\"]}|pdf"
  "pdf_alternate_mix|{\"a\":\"$P\",\"b\":\"$O\",\"reverse_b\":true}|pdf"
  "pdf_split_pages|{\"input\":\"$P\",\"ranges\":[{\"start\":0,\"end\":0},{\"start\":1,\"end\":2}]}|pdf"
  "pdf_split_size|{\"input\":\"$P\",\"max_bytes\":1000000000}|pdf"
  "pdf_split_text|{\"input\":\"$P\",\"text\":\"two\"}|pdf"
  "pdf_split_bookmarks|{\"input\":\"$O\"}|pdf"
  "pdf_nup|{\"input\":\"$P\",\"per_sheet\":4}|pdf"
  "pdf_crop|{\"input\":\"$P\",\"pages\":[0],\"rect\":$RECT}|pdf"
  "pdf_insert_blank|{\"input\":\"$P\",\"after\":[1]}|pdf"
  "pdf_split_half|{\"input\":\"$P\"}|pdf"
  "pdf_extract_images|{\"input\":\"$IMG\"}|png"
  "pdf_page_numbers|{\"input\":\"$P\"}|pdf"
  "pdf_header_footer|{\"input\":\"$P\",\"opts\":{\"header\":{\"center\":\"T\"}}}|pdf"
  "pdf_watermark|{\"input\":\"$P\",\"opts\":{\"text\":\"W\"}}|pdf"
  "pdf_bates|{\"input\":\"$P\"}|pdf"
  "pdf_compress|{\"input\":\"$IMG\",\"opts\":{\"dpi\":72}}|pdf"
  "pdf_set_metadata|{\"input\":\"$P\",\"metadata\":{\"title\":\"T\"}}|pdf"
  "pdf_flatten|{\"input\":\"$AN\"}|pdf"
  "pdf_delete_annotations|{\"input\":\"$AN\",\"pages\":[0]}|pdf"
  "pdf_repair|{\"input\":\"$P\"}|pdf"
  "pdf_linearize|{\"input\":\"$P\"}|pdf"
  "pdf_encrypt|{\"input\":\"$P\",\"opts\":{\"owner_pw\":\"o\",\"permissions\":{\"print\":\"none\"}}}|pdf"
  "pdf_decrypt|{\"input\":\"$ENC\",\"password\":\"user\"}|pdf"
  "pdf_unlock|{\"input\":\"$OWN\"}|pdf"
  "pdf_inspect_security|{\"input\":\"$OWN\"}|json"
  "pdf_redact|{\"input\":\"$P\",\"boxes\":[{\"page\":1,\"rect\":$RECT}]}|pdf"
  "pdf_ocr|{\"input\":\"$INV\"}|pdf"
  "pdf_list_fields|{\"input\":\"$FORM\"}|fields"
  "pdf_fill_forms|{\"input\":\"$FORM\",\"values\":[{\"name\":\"email\",\"value\":\"a@b\"}]}|pdf"
  "pdf_sign_image|{\"input\":\"$P\",\"image\":\"$SIG\",\"placement\":{\"page\":0,\"rect\":$RECT}}|pdf"
  "pdf_sign_text|{\"input\":\"$P\",\"text\":\"Ann\",\"font_size\":12,\"placement\":{\"page\":0,\"rect\":$RECT}}|pdf"
  "pdf_annotate|{\"input\":\"$P\",\"annots\":[{\"page\":0,\"annot\":{\"type\":\"highlight\",\"rect\":$RECT,\"color\":\"#FFFF00\"}}]}|pdf"
  "pdf_images_to_pdf|{\"inputs\":[\"$SIG\",\"$SIG\"],\"page_size\":{\"width\":612,\"height\":792}}|pdf"
  "pdf_to_images|{\"input\":\"$P\",\"dpi\":72}|png"
  "pdf_to_images|{\"input\":\"$P\",\"format\":\"jpeg\",\"pages\":[0]}|jpeg"
  "pdf_rotate|{\"input\":\"$ENC\",\"pages\":[0],\"degrees\":90,\"password\":\"user\"}|pdf"
  "pdf_office_to_pdf|{\"input\":\"$W/hello.docx\"}|pdf"
)

for row in "${ROWS[@]}"; do
  IFS='|' read -r tool args kind <<<"$row"
  step "$tool"
  call "$tool" "$args"
  id="$(get result.structuredContent.job_id)"
  state=""
  for _ in $(seq 600); do
    call pdf_job_status "{\"job_id\":\"$id\"}"
    state="$(get result.structuredContent.state.kind)"
    [[ "$state" == queued || "$state" == running ]] || break
    sleep 0.1
  done
  [[ "$state" == done ]] || fail "job ended $state: $(get result.structuredContent.state.message 2>/dev/null)"
  n="$(get result.structuredContent.outputs)"
  (( n > 0 )) || fail "no outputs"
  for i in $(seq 0 $((n - 1))); do
    out="$(get "result.structuredContent.outputs.$i")"
    [[ -f "$out" ]] || fail "output missing: $out"
    case "$kind" in
      pdf) clean "$QPDF" --check "$out" >/dev/null || fail "invalid PDF: $out" ;;
      png) file -b "$out" | grep -q '^PNG image' || fail "not a PNG: $out" ;;
      jpeg) file -b "$out" | grep -q '^JPEG image' || fail "not a JPEG: $out" ;;
      json) plutil -extract encrypted raw -o - "$out" >/dev/null || fail "not a security report: $out" ;;
      fields) plutil -extract fields.0.name raw -o - "$out" >/dev/null || fail "not a field list: $out" ;;
    esac
  done
  echo "   $n output(s) ok"
done

step "app still running"
kill -0 "$PID" 2>/dev/null || fail "app exited during the run"
echo "SMOKE OK: $(basename "$DMG") ($(du -h "$DMG" | awk '{print $1}'))"
