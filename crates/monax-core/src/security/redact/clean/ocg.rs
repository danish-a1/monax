//! R106 / R108 / R110: a group dictionary cut to what decides its
//! visibility (type, name, intent) plus the view state its /Usage forces in
//! pdfium, compared by decoded text (a name or a text string alike).
use crate::content::qjson::Doc;
use serde_json::{Value, json};

/// `v` (references resolved) as text: a name without its slash, or a text
/// string decoded; `None` for anything else.
///
/// ponytail: qpdf JSON decodes strings, so pdfium's byte compare is not
/// reproduced: a UTF-16 "View" / "OFF" reads like a PDFDoc one, and a
/// dangling /ViewState (dropped by qpdf) reads as absent. Both only ever
/// hide what pdfium shows, never reveal (R110); read the source bytes if
/// that fidelity is ever needed.
pub(super) fn text<'a>(doc: &'a Doc, v: &'a Value) -> Option<&'a str> {
    let s = doc.get(v).as_str()?;
    s.strip_prefix('/').or_else(|| s.strip_prefix("u:"))
}

/// Group `g` cut to /Type, /Name (a text string; empty when it is none)
/// and /Intent (the /View, /Design and /All among its names and strings,
/// as names; `[]` when a single value is none of them, which pdfium reads
/// alike), with the view state its /Usage /View /ViewState forces (pdfium
/// honours it before /D): `Some(false)` for "OFF", `Some(true)` for any
/// other value. `None` unless `g` is a dictionary.
pub(super) fn group(doc: &Doc, g: &str) -> Option<(Value, Option<bool>)> {
    let d = super::obj(doc, g);
    d.as_object()?;
    let name = doc.get(&d["/Name"]);
    let is_text = name
        .as_str()
        .is_some_and(|s| s.starts_with("u:") || s.starts_with("b:"));
    let mut v =
        json!({ "/Type": "/OCG", "/Name": if is_text { name.clone() } else { "u:".into() } });
    let intent = |x: &Value| -> Option<Value> {
        let t = text(doc, x)?;
        let n = ["View", "Design", "All"].into_iter().find(|n| *n == t)?;
        Some(format!("/{n}").into())
    };
    match doc.get(&d["/Intent"]) {
        Value::Null => {}
        Value::Array(a) => v["/Intent"] = a.iter().filter_map(intent).collect::<Vec<_>>().into(),
        i => v["/Intent"] = intent(i).unwrap_or_else(|| json!([])),
    }
    let view = doc.get(&doc.get(&d["/Usage"])["/View"]);
    let state = view.get("/ViewState").map(|s| text(doc, s) != Some("OFF"));
    Some((v, state))
}
