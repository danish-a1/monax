//! The qpdf JSON update that adds built annotations to their pages: each
//! annotation and its appearance become new objects, and each page that
//! gets one is rewritten with its /Annots extended (as a direct array, so
//! an array another page shares is never changed).
use super::appearance::Built;
use crate::content::qjson::{Doc, stream};
use crate::pages::PagesError;
use crate::pdfium::PdfiumError;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

/// The objects to write for `built` (0-based page, annotation) in `doc`,
/// whose pages pdfium counts `count`.
///
/// # Errors
/// `Pdfium(DamagedRepairFirst)` when qpdf's page tree does not list
/// `count` pages (a page listed twice is listed once there), `Damaged`
/// when a page's /Annots is not an array.
pub(super) fn updates(
    doc: &Doc,
    count: usize,
    built: Vec<(usize, Built)>,
) -> Result<Map<String, Value>, PagesError> {
    let (pages, _) = doc.page_tree();
    if pages.len() != count {
        return Err(PdfiumError::DamagedRepairFirst.into());
    }
    let mut out = Map::new();
    let mut added: BTreeMap<usize, Vec<Value>> = BTreeMap::new();
    let mut id = doc.new_id();
    for (page, b) in built {
        let at = pages.get(page).ok_or(PagesError::Damaged)?;
        let (annot, ap) = (format!("{id} 0 R"), format!("{} 0 R", id + 1));
        id += 2;
        let mut dict = b.dict;
        dict.insert("/P".into(), json!(at));
        dict.insert("/AP".into(), json!({ "/N": ap }));
        out.insert(format!("obj:{annot}"), json!({ "value": dict }));
        out.insert(format!("obj:{ap}"), stream(b.form, &b.content));
        added.entry(page).or_default().push(json!(annot));
    }
    for (page, new) in added {
        let at = &pages[page];
        let mut dict = page_dict(doc, at)?;
        let mut list = existing(doc, &dict)?;
        list.extend(new);
        dict.insert("/Annots".into(), Value::Array(list));
        out.insert(format!("obj:{at}"), json!({ "value": dict }));
    }
    Ok(out)
}

fn page_dict(doc: &Doc, at: &str) -> Result<Map<String, Value>, PagesError> {
    let r = Value::from(at);
    let page = doc.get(&r).as_object().cloned();
    page.ok_or(PagesError::Damaged)
}

/// The current /Annots entries of the page `dict` (none when it has none).
fn existing(doc: &Doc, dict: &Map<String, Value>) -> Result<Vec<Value>, PagesError> {
    match doc.get(dict.get("/Annots").unwrap_or(&Value::Null)) {
        Value::Array(a) => Ok(a.clone()),
        Value::Null => Ok(Vec::new()),
        _ => Err(PagesError::Damaged),
    }
}

#[cfg(test)]
mod tests;
