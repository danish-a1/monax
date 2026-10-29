"""Dev/build tool: writes THIRD-PARTY.txt from cargo / npm metadata.
Usage: third_party.py OUT META.json TREE.txt NPM.json | third_party.py --self-check
Fails (exit 1) on any GPL/AGPL/LGPL-only license expression."""
import json, os, re, sys

LICENSE_PREFIXES = ("LICENSE", "LICENCE", "COPYING", "NOTICE")
IJG = ("jpeg-encoder: this software is based in part on the work of the Independent JPEG Group.\n"
       "The IJG acknowledgement is required by jpeg-encoder's license (IJG-derived code).\n")


def copyleft_only(expr):
    """True when every OR alternative contains a (A|L)GPL term; None/unknown is not flagged."""
    alts = re.split(r"\s+OR\s+|\s*/\s*", expr or "", flags=re.I)
    bad = [any(re.match(r"^\(?(A|L)?GPL", t.strip("() "), re.I) for t in re.split(r"\s+AND\s+", a, flags=re.I)) for a in alts]
    return bool(expr) and all(bad)


def license_files(src):
    return sorted(f for f in os.listdir(src)
                  if f.upper().startswith(LICENSE_PREFIXES) and os.path.isfile(os.path.join(src, f)))


def entry(kind, name, version, lic, repo, src):
    out = ["=" * 78, f"{name} {version} ({kind})", f"License: {lic}"]
    files = license_files(src) if src and os.path.isdir(src) else []
    if not files:
        out.append(f"No license file shipped. Repository: {repo}")
    for f in files:
        out += ["", f"--- {f} ---", open(os.path.join(src, f), errors="replace").read().rstrip()]
    return "\n".join(out) + "\n"


def walk(deps, seen):
    for name, d in (deps or {}).items():
        if d.get("path") and d["path"] not in seen:
            seen[d["path"]] = (name, d.get("version", "?"))
        walk(d.get("dependencies"), seen)


def main(out, meta, tree, npm):
    used = {tuple(l.split()[:2]) for l in open(tree).read().splitlines() if l.strip()}
    parts, flags, bad, ncrate = [], [], [], 0
    for p in sorted(json.load(open(meta))["packages"], key=lambda p: (p["name"], p["version"])):
        if not p["source"] or (p["name"], "v" + p["version"]) not in used:
            continue  # workspace crate, or not in the normal closure
        ncrate += 1
        lic = p["license"] or "UNKNOWN"
        parts.append(entry("crate", p["name"], p["version"], lic, p["repository"], os.path.dirname(p["manifest_path"])))
        (bad if copyleft_only(lic) else flags if re.search("LGPL|MPL", lic) else []).append(f"crate {p['name']} {p['version']}: {lic}")
    seen = {}
    walk(json.load(open(npm)).get("dependencies"), seen)
    for path, (name, ver) in sorted(seen.items(), key=lambda kv: kv[1]):
        pj = json.load(open(os.path.join(path, "package.json")))
        lic = pj.get("license") or "UNKNOWN"
        if not isinstance(lic, str):
            lic = str(lic.get("type"))
        parts.append(entry("npm", name, ver, lic, pj.get("homepage") or pj.get("repository"), path))
        (bad if copyleft_only(lic) else flags if re.search("LGPL|MPL", lic) else []).append(f"npm {name} {ver}: {lic}")
    for f in flags:
        print("NOTE copyleft-adjacent license:", f)
    if bad:
        print("ERROR copyleft-only license in the bundled app:\n  " + "\n  ".join(bad), file=sys.stderr)
        sys.exit(1)
    with open(out, "w") as f:
        f.write("Third-party software in Monax\n\n" + IJG + "\n" + "\n".join(parts))
    print(f"THIRD-PARTY.txt: {ncrate} crates, {len(seen)} npm packages")


def self_check():
    assert copyleft_only("GPL-3.0-only") and copyleft_only("LGPL-2.1-or-later") and copyleft_only("AGPL-3.0")
    assert copyleft_only("GPL-2.0 AND LGPL-2.1")
    assert not copyleft_only("MIT OR GPL-3.0") and not copyleft_only("MIT") and not copyleft_only("MPL-2.0")
    assert not copyleft_only("(MIT OR Apache-2.0) AND Unicode-3.0")
    print("self-check ok")


if __name__ == "__main__":
    self_check() if sys.argv[1:] == ["--self-check"] else main(*sys.argv[1:])
