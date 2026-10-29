#!/usr/bin/env bash
# Dev/build tool: WordCraft license files -> sidecars/licenses/wordcraft/. Run by fetch-sidecars.sh
# after the WordCraft build, or alone (needs only the vendor/wordcraft source tree, no target dir).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${OUT:-$ROOT/sidecars}"
WC="${WC:-$ROOT/vendor/wordcraft}"
mkdir -p "$OUT/licenses/wordcraft/crates"
cp "$WC"/LICENSE-MIT "$WC"/LICENSE-APACHE "$WC"/NOTICE "$WC"/ATTRIBUTION.md "$WC"/assets/fonts/OFL-*.txt \
  "$OUT/licenses/wordcraft/"
# License files of every crate compiled into it -> licenses/wordcraft/crates/<name>-<version>/.
(cd "$WC" && cargo metadata --locked --format-version 1) | python3 -I -c '
import json, os, shutil, sys
tree, out = sys.argv[1], sys.argv[2]
used = {tuple(l.split()[:2]) for l in tree.splitlines()}
for p in json.load(sys.stdin)["packages"]:
    if (p["name"], "v" + p["version"]) not in used:
        continue
    src, dst = os.path.dirname(p["manifest_path"]), os.path.join(out, p["name"] + "-" + p["version"])
    files = [f for f in os.listdir(src) if f.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE"))]
    if not p["source"]:
        continue  # a WordCraft workspace crate: its licenses are the root ones
    os.makedirs(dst, exist_ok=True)
    # Some crates ship no license file: their declared license and home stand in.
    with open(os.path.join(dst, "PACKAGE"), "w") as f:
        f.write(p["name"] + " " + p["version"] + ": " + str(p["license"]) + " " + str(p["repository"]) + "\n")
    for f in files:
        (shutil.copytree if os.path.isdir(os.path.join(src, f)) else shutil.copy)(os.path.join(src, f), os.path.join(dst, f))
' "$(cd "$WC" && cargo tree --locked -p wordcraft-cli -e normal --prefix none)" "$OUT/licenses/wordcraft/crates"
