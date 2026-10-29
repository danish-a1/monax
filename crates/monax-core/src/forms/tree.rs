//! The field tree as qpdf reads it: terminal fields with their widgets.
use super::FieldKind;
use super::node::{Inherited, Node, list, node};
use super::text::text;
use crate::content::qjson::Doc;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// pdfium nests fields at most 32 deep; deeper ones are not fields to it.
const DEPTH: usize = 32;

/// Every terminal field of `doc`'s form, in field-tree order. Fields are
/// indirect objects (PDF 32000 12.7.4); one reached twice, a direct one or
/// one without /FT is not listed.
pub(super) fn nodes(doc: &Doc) -> Vec<Node> {
    let form = doc.get(&doc.get(&doc.trailer()["/Root"])["/AcroForm"]);
    let pages = widget_pages(doc);
    let mut todo: Vec<(&Value, Inherited, usize)> = list(doc, &form["/Fields"])
        .iter()
        .rev()
        .map(|f| (f, Inherited::default(), 0))
        .collect();
    let (mut out, mut seen) = (Vec::new(), HashSet::new());
    while let Some((f, up, depth)) = todo.pop() {
        let Some(r) = f.as_str().filter(|r| r.ends_with(" R")) else {
            continue;
        };
        if depth > DEPTH || !seen.insert(r) {
            continue;
        }
        let d = doc.get(f);
        let mut inh = up.clone();
        if let Some(t) = text(&d["/T"]) {
            inh.name = if up.name.is_empty() {
                t
            } else {
                format!("{}.{t}", up.name)
            };
        }
        inh.ft = d["/FT"].as_str().or(up.ft);
        inh.ff = d["/Ff"].as_u64().unwrap_or(up.ff);
        inh.v = d.get("/V").or(up.v);
        inh.max_len = d["/MaxLen"].as_u64().map(|n| n as usize).or(up.max_len);
        let kids = list(doc, &d["/Kids"]);
        if kids.iter().any(|k| doc.get(k).get("/T").is_some()) {
            todo.extend(kids.iter().rev().map(|k| (k, inh.clone(), depth + 1)));
        } else if let Some(kind) = kind(inh.ft, inh.ff) {
            let ws = if kids.is_empty() {
                std::slice::from_ref(f)
            } else {
                kids
            };
            out.push(node(doc, r, d, kind, &inh, ws, &pages));
        }
    }
    out
}

/// The kind of a field of type `ft` with flags `ff` (PDF 32000 12.7.4).
fn kind(ft: Option<&str>, ff: u64) -> Option<FieldKind> {
    Some(match ft? {
        "/Tx" => FieldKind::Text,
        "/Ch" if ff & (1 << 17) != 0 => FieldKind::Combo,
        "/Ch" => FieldKind::List,
        "/Btn" if ff & (1 << 16) != 0 => FieldKind::Button,
        "/Btn" if ff & (1 << 15) != 0 => FieldKind::Radio,
        "/Btn" => FieldKind::Checkbox,
        "/Sig" => FieldKind::Signature,
        _ => return None,
    })
}

/// Each widget (by reference) to the 0-based page whose /Annots first
/// lists it.
fn widget_pages(doc: &Doc) -> HashMap<String, usize> {
    let mut m = HashMap::new();
    for (i, p) in doc.page_tree().0.into_iter().enumerate() {
        let p = Value::from(p);
        let page = doc.get(&p);
        for a in list(doc, &page["/Annots"]).iter().filter_map(Value::as_str) {
            m.entry(a.to_string()).or_insert(i);
        }
    }
    m
}
