//! Form test helpers: `form.pdf` (one 300 x 300 pt page: text fields
//! `person.name` = "Ann" (stale appearance) and `email` (MaxLen 10),
//! checkbox `agree` (export "Agreed"), radio `color` (red, blue), combo
//! `size` (S, M, L), list `fruit`, push button `go`, signature `sig`,
//! read-only `locked`, editable combo `note`) and copies derived from it
//! with qpdf.
use super::copy_fixture;
use super::docs::{get, objects, qpdf};
use monax_core::forms::{Field, FieldValue, list_fields};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// `form.pdf` copied into a fresh temp dir.
pub fn form() -> (TempDir, PathBuf) {
    copy_fixture("form.pdf", "form.pdf")
}

pub fn fv(name: &str, value: &str) -> FieldValue {
    FieldValue {
        name: name.into(),
        value: value.into(),
    }
}

/// The listed field named `name`.
pub fn field(p: &Path, name: &str) -> Field {
    let all = list_fields(p).unwrap();
    all.into_iter().find(|f| f.name == name).unwrap()
}

/// `src` with qpdf JSON object `updates` applied, written as `name` beside it.
pub fn derive(src: &Path, name: &str, updates: Value) -> PathBuf {
    let dst = src.with_file_name(name);
    let upd = src.with_file_name(format!("{name}.json"));
    let head = json!({ "jsonversion": 2, "pushedinheritedpageresources": false,
        "calledgetallpages": false, "maxobjectid": 40 });
    std::fs::write(&upd, json!({ "qpdf": [head, updates] }).to_string()).unwrap();
    let arg = format!("--update-from-json={}", upd.display());
    qpdf(&[src.to_str().unwrap(), &arg, dst.to_str().unwrap()]);
    std::fs::remove_file(upd).unwrap();
    dst
}

/// `form.pdf`'s catalog value with `extra` entries added to its AcroForm.
pub fn catalog_with(src: &Path, extra: Value) -> Value {
    let objs = objects(src);
    let mut cat = get(&objs, &json!("1 0 R")).clone();
    for (k, v) in extra.as_object().unwrap() {
        cat["/AcroForm"][k] = v.clone();
    }
    json!({ "obj:1 0 R": { "value": cat } })
}

/// The widget dictionary whose /T is `t` (qpdf JSON).
pub fn widget(p: &Path, t: &str) -> Value {
    let objs = objects(p);
    let want = json!(format!("u:{t}"));
    objs.values()
        .map(|o| &o["value"])
        .find(|v| v["/T"] == want)
        .cloned()
        .unwrap()
}

/// Whether the 72 dpi renders of page 0 of `a` and `b` (300 x 300 pt,
/// unrotated) differ inside the user space rect `l, b, r, t`.
pub fn ink_in(a: &Path, b: &Path, [l, bo, r, t]: [u32; 4]) -> bool {
    super::diff_box_in(a, b, 0, l..r, (300 - t)..(300 - bo)).is_some()
}
