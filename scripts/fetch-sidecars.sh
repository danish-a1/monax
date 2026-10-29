#!/usr/bin/env bash
# Dev/build tool (not app runtime): lays out self-contained qpdf + tesseract + wordcraft-cli in
# sidecars/, plus licenses/ for every bundled binary and pdfium (from vendor/pdfium).
# No static macOS qpdf release exists, so qpdf and tesseract are copied from Homebrew with their
# dylibs. wordcraft-cli (pure Rust, MIT OR Apache-2.0) is built from source at a pinned rev in the
# gitignored vendor/wordcraft (the only network use: git clone and cargo's crate fetch).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/sidecars"
BREW="$(brew --prefix)"
TESSDATA="$BREW/share/tessdata"
PDFIUM="$ROOT/vendor/pdfium"
WC="$ROOT/vendor/wordcraft"
WC_REV=aadb077e072b7de05f724e359d8db82e151f125c
shopt -s nullglob

rm -rf "$OUT"
mkdir -p "$OUT/lib" "$OUT/tessdata" "$OUT/licenses"
cp "$BREW/bin/qpdf" "$BREW/bin/tesseract" "$OUT/"
cp "$TESSDATA/eng.traineddata" "$OUT/tessdata/"
chmod u+w "$OUT"/qpdf "$OUT"/tesseract

# Non-system dependencies of a Mach-O file (resolved to real paths where possible).
deps() { otool -L "$1" | tail -n +2 | awk '{print $1}' | grep -Ev '^(/usr/lib|/System|@loader_path)' || true; }

# Real path of a dependency: absolute as-is, @rpath/x searched in Homebrew lib dirs.
resolve() {
  local dep="$1" c
  if [[ "$dep" == /* ]]; then realpath "$dep"; return; fi
  for c in "$BREW/lib/${dep#@rpath/}" "$BREW"/opt/*/lib/"${dep#@rpath/}"; do
    [[ -f "$c" ]] && { realpath "$c"; return; }
  done
  echo "ERROR: cannot resolve $dep" >&2; return 1
}

# License files of the Homebrew keg holding real path $1 -> licenses/<formula>/.
license() {
  local keg pkg dst f
  keg="$(sed -E 's#(/Cellar/[^/]+/[^/]+)/.*#\1#' <<<"$1")"
  pkg="$(basename "$(dirname "$keg")")"
  dst="$OUT/licenses/$pkg"
  [[ -d "$dst" ]] && return
  mkdir "$dst"
  for f in "$keg"/LICENSE* "$keg"/COPYING* "$keg"/NOTICE*; do cp "$f" "$dst/"; done
  # ponytail: leptonica's keg ships no license file; its BSD-2 text heads every header.
  if [[ "$pkg" == leptonica ]]; then
    sed -n '1,/^ \*=*\*\/$/p' "$keg/include/leptonica/environ.h" >"$dst/LICENSE"
  fi
  [[ -n "$(ls "$dst")" ]] || { echo "ERROR: no license file in $keg" >&2; exit 1; }
}
license "$(realpath "$BREW/bin/qpdf")"
license "$(realpath "$BREW/bin/tesseract")"
# eng.traineddata (tesseract-ocr/tessdata_fast via Homebrew) is Apache-2.0, as tesseract.
mkdir "$OUT/licenses/tessdata"
cp "$OUT/licenses/tesseract/LICENSE" "$OUT/licenses/tessdata/LICENSE"
echo "eng.traineddata: tesseract-ocr/tessdata_fast, Apache License 2.0" >"$OUT/licenses/tessdata/NOTICE"

# Recursively copy every non-system dylib into lib/.
collect() {
  local dep real base
  for dep in $(deps "$1"); do
    real="$(resolve "$dep")"
    base="$(basename "$real")"
    if [[ ! -f "$OUT/lib/$base" ]]; then
      cp "$real" "$OUT/lib/$base"
      chmod u+w "$OUT/lib/$base"
      license "$real"
      collect "$OUT/lib/$base"
    fi
  done
}
collect "$OUT/qpdf"
collect "$OUT/tesseract"

[[ -f "$PDFIUM/LICENSE" ]] || { echo "ERROR: $PDFIUM/LICENSE missing" >&2; exit 1; }
mkdir "$OUT/licenses/pdfium"
cp -R "$PDFIUM/LICENSE" "$PDFIUM/licenses" "$OUT/licenses/pdfium/"

# WordCraft at the pinned rev. Gate: no HTTP or update client may reach the CLI (R152).
[[ -d "$WC/.git" ]] || git clone --quiet https://github.com/storytold/wordcraft "$WC"
git -C "$WC" cat-file -e "$WC_REV^{commit}" 2>/dev/null || git -C "$WC" fetch --quiet origin
git -C "$WC" checkout --quiet --detach "$WC_REV"
WC_TREE="$(cd "$WC" && cargo tree --locked -p wordcraft-cli -e normal)"
NET='craft-update|reqwest|ureq|hyper|isahc|curl|attohttpc|surf|h2|tungstenite|rustls|native-tls|openssl'
if grep -Ew "$NET" <<<"$WC_TREE"; then
  echo "ERROR: wordcraft-cli depends on an HTTP / update client (above)" >&2; exit 1
fi
(cd "$WC" && cargo build --quiet --release --locked -p wordcraft-cli)
cp "$WC/target/release/wordcraft-cli" "$OUT/"
chmod u+w "$OUT/wordcraft-cli"
OUT="$OUT" WC="$WC" "$ROOT/scripts/wordcraft-licenses.sh"

# Rewrite install names: executables point at @loader_path/lib, dylibs at siblings.
fix() {
  local file="$1" prefix="$2" dep base
  for dep in $(deps "$file"); do
    base="$(basename "$(resolve "$dep")")"
    install_name_tool -change "$dep" "$prefix/$base" "$file"
  done
}
for b in qpdf tesseract; do fix "$OUT/$b" "@loader_path/lib"; done
for d in "$OUT"/lib/*.dylib; do
  install_name_tool -id "@loader_path/$(basename "$d")" "$d"
  fix "$d" "@loader_path"
done

# Drop Homebrew-relative rpaths.
for f in "$OUT"/qpdf "$OUT"/tesseract "$OUT"/lib/*.dylib; do
  for r in $(otool -l "$f" | awk '/LC_RPATH/{g=1} g&&/ path /{print $2; g=0}'); do
    install_name_tool -delete_rpath "$r" "$f"
  done
done

# install_name_tool invalidates signatures: re-sign ad hoc, dylibs before executables.
for f in "$OUT"/lib/*.dylib "$OUT"/qpdf "$OUT"/tesseract "$OUT"/wordcraft-cli; do
  codesign -s - -f --timestamp=none "$f"
done

"$ROOT/scripts/third-party.sh"

# Fail loudly on any leftover Homebrew / local reference or unresolved @rpath.
for f in "$OUT"/qpdf "$OUT"/tesseract "$OUT"/wordcraft-cli "$OUT"/lib/*.dylib; do
  if otool -L "$f" | tail -n +2 | grep -Eq '/opt/homebrew/|/usr/local/|@rpath/'; then
    echo "ERROR: $f references a library outside the bundle" >&2
    otool -L "$f" >&2
    exit 1
  fi
done

# wordcraft-cli is pure Rust: only system libraries, nothing bundled beside it.
if otool -L "$OUT/wordcraft-cli" | tail -n +2 | awk '{print $1}' | grep -Ev '^(/usr/lib/|/System/)'; then
  echo "ERROR: wordcraft-cli links a non-system library (above)" >&2; exit 1
fi

{
  echo "qpdf: $("$OUT/qpdf" --version | head -1)"
  echo "tesseract: $(TESSDATA_PREFIX="$OUT/tessdata" "$OUT/tesseract" --version 2>&1 | head -1)"
  echo "tessdata: eng"
  echo "wordcraft: $("$OUT/wordcraft-cli" --version | head -1) (storytold/wordcraft@$WC_REV)"
  echo "licenses: $(ls "$OUT/licenses" | tr '\n' ' ')"
} | tee "$OUT/MANIFEST.txt"
