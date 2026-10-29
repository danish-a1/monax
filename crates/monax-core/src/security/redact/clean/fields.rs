//! The form field tree after a redaction: widgets made unreachable leave
//! their fields; a field left with no kids goes too.
use super::obj;
use crate::content::qjson::Doc;
use serde_json::{Map, Value, json};
use std::collections::HashSet;

/// `form` (an AcroForm dictionary) with /Fields and /CO free of `dead`
/// objects and of fields that lose every kid; those fields join `dead`, and
/// fields that lose some kids are rewritten in `up`. A button field whose
/// value or default (/V, /DV) names a state no remaining widget has is set
/// to /Off, and the dropped kids' /Opt entries (export values, one per kid)
/// become empty strings: kept on-state names are /Opt indices. `None` when
/// the form is unchanged.
pub(super) fn prune(
    doc: &Doc,
    form: &Value,
    dead: &mut HashSet<String>,
    up: &mut Map<String, Value>,
) -> Option<Value> {
    let list = |k: &str| doc.get(&form[k]).as_array().cloned().unwrap_or_default();
    let (fields, co) = (list("/Fields"), list("/CO"));
    let seen = &mut HashSet::new();
    let kept: Vec<Value> = fields
        .iter()
        .filter(|f| alive(doc, f, dead, up, seen, 0))
        .cloned()
        .collect();
    let gone = |e: &&Value| e.as_str().is_some_and(|r| dead.contains(r));
    let co_kept: Vec<Value> = co.iter().filter(|e| !gone(e)).cloned().collect();
    if kept.len() == fields.len() && co_kept.len() == co.len() {
        return None;
    }
    let mut f = form.as_object()?.clone();
    f.insert("/Fields".into(), Value::Array(kept));
    if f.contains_key("/CO") {
        f.insert("/CO".into(), Value::Array(co_kept));
    }
    Some(Value::Object(f))
}

/// Whether field or widget `f` survives (see [`prune`]). Each field is
/// decided once (`seen`; one seen again and not dead is kept, a cycle
/// too), so a kids DAG takes linear time; the recursion stops at depth 32.
fn alive(
    doc: &Doc,
    f: &Value,
    dead: &mut HashSet<String>,
    up: &mut Map<String, Value>,
    seen: &mut HashSet<String>,
    depth: u8,
) -> bool {
    let Some(r) = f.as_str() else { return true }; // a direct field stays
    if dead.contains(r) {
        return false;
    }
    let d = obj(doc, r);
    let kids = doc.get(&d["/Kids"]).as_array().cloned().unwrap_or_default();
    if kids.is_empty() || depth > 32 || !seen.insert(r.to_string()) {
        return true;
    }
    let keep: Vec<bool> = kids
        .iter()
        .map(|k| alive(doc, k, dead, up, seen, depth + 1))
        .collect();
    let pick = |v: &[Value]| -> Vec<Value> {
        v.iter()
            .zip(&keep)
            .filter(|(_, k)| **k)
            .map(|(x, _)| x.clone())
            .collect()
    };
    let left = pick(&kids);
    if left.is_empty() {
        dead.insert(r.to_string());
        return false;
    }
    if left.len() < kids.len() {
        let mut d = d.as_object().cloned().unwrap_or_default();
        let states: HashSet<String> = left
            .iter()
            .flat_map(|k| {
                let n = &doc.get(&doc.get(k)["/AP"])["/N"];
                doc.get(n).as_object().cloned().unwrap_or_default()
            })
            .map(|(s, _)| s)
            .collect();
        // A button value or default naming a state only a dropped widget had.
        for key in ["/V", "/DV"] {
            let v = d.get(key).and_then(Value::as_str).unwrap_or_default();
            if v.starts_with('/') && v != "/Off" && !states.contains(v) {
                d.insert(key.into(), Value::from("/Off"));
            }
        }
        // /Opt holds one export value per kid, in kid order; kept widgets'
        // state names may be indices into it, so it keeps its length.
        let opt = doc
            .get(&d.get("/Opt").cloned().unwrap_or_default())
            .as_array()
            .cloned();
        if let Some(opt) = opt.filter(|o| o.len() == kids.len()) {
            let blank = |(x, k): (&Value, &bool)| if *k { x.clone() } else { json!("u:") };
            d.insert("/Opt".into(), opt.iter().zip(&keep).map(blank).collect());
        }
        d.insert("/Kids".into(), Value::Array(left));
        up.insert(format!("obj:{r}"), json!({ "value": d }));
    }
    true
}

/// Whether `form` still has fields and a /DR font among `dead`: kept fields
/// that used it are drawn in another font when their appearance is redone.
pub(super) fn lost_font(doc: &Doc, form: &Value, dead: &HashSet<String>) -> bool {
    let fields = doc
        .get(&form["/Fields"])
        .as_array()
        .is_some_and(|f| !f.is_empty());
    let fonts = doc
        .get(&doc.get(&form["/DR"])["/Font"])
        .as_object()
        .cloned();
    let gone = |f: &Value| f.as_str().is_some_and(|r| dead.contains(r));
    fields && fonts.unwrap_or_default().values().any(gone)
}
