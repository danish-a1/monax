//! Document builders and qpdf-based readers for the document-level ops.
use monax_sidecar::{Tool, run};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

fn qpdf_run(args: &[&str]) -> monax_sidecar::RunOutput {
    let args: Vec<_> = args.iter().map(Into::into).collect();
    let never = AtomicBool::new(false);
    run(Tool::Qpdf, &args, Duration::from_secs(60), &never).unwrap()
}

/// Runs qpdf with `args`; panics unless it exits 0 or 3. Returns stdout.
pub fn qpdf(args: &[&str]) -> String {
    let out = qpdf_run(args);
    assert!(matches!(out.status.code(), Some(0 | 3)), "{}", out.stderr);
    out.stdout
}

/// Panics unless `qpdf --check` finds no damage and no warnings.
pub fn assert_clean(p: &Path) {
    let out = qpdf_run(&["--check", s(p)]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}: {}",
        p.display(),
        out.stderr
    );
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

/// qpdf JSON v2 object map ("obj:N G R" / "trailer" -> {value | stream}).
pub fn objects(p: &Path) -> serde_json::Map<String, Value> {
    let v: Value = serde_json::from_str(&qpdf(&["--json=2", "--json-key=qpdf", s(p)])).unwrap();
    v["qpdf"][1].as_object().unwrap().clone()
}

/// `v` itself, or the object it references ("N G R"); a stream's dict.
pub fn get<'a>(objs: &'a serde_json::Map<String, Value>, v: &'a Value) -> &'a Value {
    match v.as_str().filter(|r| r.ends_with(" R")) {
        Some(r) => {
            let o = &objs[&format!("obj:{r}")];
            if o["value"].is_null() {
                &o["stream"]["dict"]
            } else {
                &o["value"]
            }
        }
        None => v,
    }
}

/// The Info dictionary's string entries without qpdf's `u:` prefix.
pub fn info(p: &Path) -> Vec<(String, String)> {
    let objs = objects(p);
    let info = get(&objs, &objs["trailer"]["value"]["/Info"]);
    let entries = info.as_object().into_iter().flatten();
    let text = |v: &Value| {
        v.as_str()
            .unwrap_or_default()
            .trim_start_matches("u:")
            .to_string()
    };
    entries.map(|(k, v)| (k.clone(), text(v))).collect()
}

/// /Subtype of every annotation, per page.
pub fn annots(p: &Path) -> Vec<Vec<String>> {
    let objs = objects(p);
    let pages: Value =
        serde_json::from_str(&qpdf(&["--json=2", "--json-key=pages", s(p)])).unwrap();
    let pages = pages["pages"].as_array().unwrap();
    pages
        .iter()
        .map(|pg| {
            let page = get(&objs, &pg["object"]);
            let list = get(&objs, &page["/Annots"])
                .as_array()
                .cloned()
                .unwrap_or_default();
            list.iter()
                .map(|a| get(&objs, a)["/Subtype"].as_str().unwrap().to_string())
                .collect()
        })
        .collect()
}

/// (Width, Height) of every image XObject, sorted.
pub fn image_sizes(p: &Path) -> Vec<(u64, u64)> {
    let mut v: Vec<_> = objects(p)
        .values()
        .map(|o| &o["stream"]["dict"])
        .filter(|d| d["/Subtype"] == "/Image")
        .map(|d| {
            (
                d["/Width"].as_u64().unwrap(),
                d["/Height"].as_u64().unwrap(),
            )
        })
        .collect();
    v.sort();
    v
}

/// Names of the embedded files.
pub fn attachments(p: &Path) -> Vec<String> {
    let v: Value =
        serde_json::from_str(&qpdf(&["--json=2", "--json-key=attachments", s(p)])).unwrap();
    v["attachments"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect()
}

/// `p` with `note.txt` embedded, written beside it as `att.pdf`.
pub fn with_attachment(p: &Path) -> PathBuf {
    let dir = p.parent().unwrap();
    let note = dir.join("note.txt");
    std::fs::write(&note, "hi").unwrap();
    let out = dir.join("att.pdf");
    qpdf(&[
        s(p),
        "--add-attachment",
        s(&note),
        "--key=note.txt",
        "--",
        s(&out),
    ]);
    std::fs::remove_file(note).unwrap();
    out
}

/// qpdf's `encrypt` JSON block of `p`, opened with `password` (test-only
/// argv password).
pub fn encryption(p: &Path, password: &str) -> Value {
    let pw = format!("--password={password}");
    let out = qpdf(&[&pw, "--json=2", "--json-key=encrypt", s(p)]);
    serde_json::from_str::<Value>(&out).unwrap()["encrypt"].clone()
}

/// `qpdf --requires-password` exit code: 0 needs one, 2 plain, 3 opens.
pub fn requires_password(p: &Path) -> Option<i32> {
    qpdf_run(&["--requires-password", s(p)]).status.code()
}
