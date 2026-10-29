//! The redaction plan, applied as object updates to the input before the
//! output is built from it and its pages alone (R99-R108): every annotation
//! listed only on redacted pages, and every object redacted pages reach
//! that no kept page draws with (`render`), set to null so nothing else a
//! kept page reaches (actions, private keys, layers) can carry them; what
//! describes a redacted page nulled, any other object naming it rewritten
//! without those entries (`cut`); form fields left without widgets dropped;
//! the catalog cut to its pages, form and kept layers, the trailer to its
//! root; notes for what goes and what stays.
mod cut;
mod dropped;
mod fields;
mod layers;
mod ocg;
mod owned;
mod render;

use crate::content::qjson::Doc;
use crate::pages::PagesError;
use serde_json::{Map, Value, json};
use std::collections::HashSet;

pub(super) const TAGS: &str = "accessibility tags were removed";
pub(super) const DESTS: &str = "links to redacted page(s) no longer have a target";
pub(super) const DROPPED: &str = "document-level features were removed: ";
pub(super) const SHARED: &str =
    "form field values and objects shared with kept pages are not redacted";
pub(super) const LAYERS: &str = "layers used only by redacted pages were removed";
pub(super) const FONTS: &str = "form fonts shared with redacted pages were removed; kept fields may show their values in another font";

/// Page object references (`"N G R"`) in page order, from the page tree.
pub(super) fn page_refs(doc: &Doc) -> Vec<String> {
    doc.page_tree().0
}

/// The value of object `r` (`"N G R"`); a stream's dictionary.
pub(super) fn obj<'a>(doc: &'a Doc, r: &str) -> &'a Value {
    let o = doc.objects.get(&format!("obj:{r}"));
    o.map_or(&Value::Null, |o| match &o["value"] {
        Value::Null => &o["stream"]["dict"],
        v => v,
    })
}

/// Annotation entries (references or direct dictionaries) of `page`.
fn annots(doc: &Doc, page: &str) -> Vec<Value> {
    let a = &obj(doc, page)["/Annots"];
    doc.get(a).as_array().cloned().unwrap_or_default()
}

/// Content stream references of `page`.
pub(super) fn contents(doc: &Doc, page: &str) -> Vec<Value> {
    let c = &obj(doc, page)["/Contents"];
    match (c, doc.get(c)) {
        (_, Value::Array(a)) => a.clone(),
        (Value::String(_), _) => vec![c.clone()],
        _ => Vec::new(),
    }
}

/// Whether `v` holds an array (outside /Kids) starting with a page of `red`:
/// a destination.
fn points_at(v: &Value, red: &HashSet<String>) -> bool {
    match v {
        Value::Array(a) => {
            a.first()
                .and_then(Value::as_str)
                .is_some_and(|r| red.contains(r))
                || a.iter().any(|x| points_at(x, red))
        }
        Value::Object(m) => m.iter().any(|(k, x)| k != "/Kids" && points_at(x, red)),
        _ => false,
    }
}

/// Object updates (qpdf JSON v2) and notes for redacting the 0-based
/// `redacted` pages of `doc`.
///
/// # Errors
/// `Damaged` when the page tree does not list a redacted page.
pub(super) fn plan(
    doc: &Doc,
    redacted: &[usize],
) -> Result<(Map<String, Value>, Vec<String>), PagesError> {
    let (pages, nodes) = doc.page_tree();
    if redacted.iter().any(|&i| i >= pages.len()) {
        return Err(PagesError::Damaged);
    }
    let (red, kept): (Vec<_>, Vec<_>) = (0..pages.len()).partition(|i| redacted.contains(i));
    let refs = |v: &[usize], f: fn(&Doc, &str) -> Vec<Value>| -> HashSet<String> {
        let all = v.iter().flat_map(|&i| f(doc, &pages[i]));
        all.filter_map(|a| a.as_str().map(String::from)).collect()
    };
    let root = doc.trailer()["/Root"].as_str().unwrap_or_default();
    // What no walk enters: pages, page tree nodes, the catalog.
    let mut stop: HashSet<String> = &nodes | &pages.iter().cloned().collect();
    stop.insert(root.to_string());
    // Annotations listed only on redacted pages (R100), and every object
    // only they reach (R101): made unreachable.
    let listed = refs(&kept, annots);
    let mut dead: HashSet<String> = &refs(&red, annots) - &listed;
    let reach = owned::owned(doc, &pages, &red, &stop, &dead);
    dead.extend(reach.owned);
    let red: HashSet<String> = red.iter().map(|&i| pages[i].clone()).collect();
    // What describes a redacted page is made unreachable too, whatever
    // reaches it; any other object naming one only loses those entries.
    let cuts = cut::objects(doc, &stop, &mut dead, &red);
    let (mut up, mut notes) = (Map::new(), Vec::new());
    let cat = obj(doc, root);
    if cat.get("/StructTreeRoot").is_some() || cat.get("/MarkInfo").is_some() {
        notes.push(TAGS.to_string());
    }
    let mut seen = reach.kept; // and the kept pages themselves
    seen.extend(kept.iter().map(|&i| pages[i].clone()));
    if seen.iter().any(|r| points_at(obj(doc, r), &red)) {
        notes.push(DESTS.to_string());
    }
    // Only the pages, the form and the kept layers stay in the catalog, so
    // a kept object naming it carries nothing else.
    let mut minimal = json!({ "/Type": "/Catalog", "/Pages": cat["/Pages"] });
    let form_ref = &cat["/AcroForm"];
    let form = doc.get(form_ref).clone();
    let mut form = fields::prune(doc, &form, &mut dead, &mut up).unwrap_or(form);
    let lost = dropped::dropped(doc, cat, &form);
    if !lost.is_empty() {
        notes.push(format!("{DROPPED}{}", lost.join(", ")));
    }
    let groups = (&reach.groups, &reach.red_groups);
    let (ocp, gone) = layers::layers(doc, root, groups, &mut up);
    if gone {
        notes.push(LAYERS.to_string());
    }
    if let Some(ocp) = ocp {
        minimal["/OCProperties"] = ocp;
    }
    if fields::lost_font(doc, &form, &dead) {
        notes.push(FONTS.to_string());
    }
    if let Some(m) = form.as_object_mut() {
        m.remove("/XFA"); // field values of every page
    }
    if form.is_object() {
        let form = cut::cut(&form, &red);
        match form_ref.as_str() {
            Some(r) => {
                up.insert(format!("obj:{r}"), json!({ "value": form }));
                minimal["/AcroForm"] = form_ref.clone();
            }
            None => minimal["/AcroForm"] = form,
        }
    }
    up.insert(format!("obj:{root}"), json!({ "value": minimal }));
    // The trailer keeps its root, size and ID only: no /Info.
    let t = doc.trailer();
    let keep = ["/Size", "/ID"].map(|k| (k.to_string(), t[k].clone()));
    let mut trailer: Map<String, Value> = keep.into_iter().filter(|(_, v)| !v.is_null()).collect();
    trailer.insert("/Root".into(), root.into());
    up.insert("trailer".into(), json!({ "value": trailer }));
    for (k, v) in cuts {
        up.entry(k).or_insert(v);
    }
    for d in &dead {
        up.insert(format!("obj:{d}"), json!({ "value": null }));
    }
    notes.push(SHARED.to_string());
    Ok((up, notes))
}

#[cfg(test)]
mod tests;
