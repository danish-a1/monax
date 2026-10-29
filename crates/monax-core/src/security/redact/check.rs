//! Last line of defence: before a redacted output is published, the
//! content it draws (page /Contents; form XObjects, tiling patterns and
//! Type3 glyph procedures reachable through page and form /Resources) and
//! every other form XObject that is not an annotation appearance (or drawn
//! by one) are searched for words that only the redacted pages showed. A
//! hit fails the job and nothing is written.
//!
//! Limits: a word is 4+ ASCII alphanumerics in the bytes of string
//! operands (strings of one TJ array joined, so kerned words stay whole),
//! matched against pdfium's text of the redacted pages minus the kept
//! pages' words. Text in CID / Identity-H fonts (bytes are glyph ids),
//! split across Tj operators, in annotation appearances, in shading or
//! image data, or in document-level strings is not seen. The structural cleanup (`clean`) is
//! the defence; this only catches what it misses in simple fonts.
// ponytail: decodes every stream of the output in memory once and checks
// word sets; stream the JSON and use a multi-pattern matcher if outputs of
// hundreds of MB or non-Latin scripts need it.
use super::clean::{contents, obj, page_refs};
use crate::content::qjson::Doc;
use crate::pages::PagesError;
use crate::pdfium;
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;

mod bytes;

use bytes::{base64, strings, words};

type Words = HashSet<Vec<u8>>;

/// Words of the 0-based `redacted` pages of `input` that no other page
/// shows: what must not be in the output.
///
/// # Errors
/// As [`pdfium::extract_text`].
pub(super) fn secret_words(input: &Path, redacted: &[usize]) -> Result<Words, PagesError> {
    let texts = pdfium::page_texts(input)?;
    let (mut secret, mut kept) = (Words::new(), Words::new());
    for (i, t) in texts.iter().enumerate() {
        let to = if redacted.contains(&i) {
            &mut secret
        } else {
            &mut kept
        };
        to.extend(words(t.as_bytes()));
    }
    secret.retain(|w| !kept.contains(w));
    Ok(secret)
}

/// References used as annotation appearances (/AP /N, /R, /D, direct or
/// per state) anywhere in `doc`.
fn appearances(doc: &Doc) -> HashSet<String> {
    fn walk(v: &Value, doc: &Doc, out: &mut HashSet<String>) {
        match v {
            Value::Object(m) => m.iter().for_each(|(k, x)| {
                if k != "/AP" {
                    return walk(x, doc, out);
                }
                for e in ["/N", "/R", "/D"].map(|s| &doc.get(x)[s]) {
                    let states = doc.get(e).as_object().into_iter().flat_map(|m| m.values());
                    let refs = std::iter::once(e).chain(states).filter_map(Value::as_str);
                    out.extend(refs.map(String::from));
                }
            }),
            Value::Array(a) => a.iter().for_each(|x| walk(x, doc, out)),
            _ => {}
        }
    }
    let mut out = HashSet::new();
    doc.objects
        .values()
        .for_each(|o| walk(&o["value"], doc, &mut out));
    out
}

/// Content streams drawn through the resource dictionaries `roots`: form
/// XObjects, tiling patterns and Type3 glyph procedures, recursively. Each
/// resource dictionary, font and stream is visited once (cycles end).
fn drawn<'a>(doc: &'a Doc, roots: Vec<&'a Value>) -> HashSet<String> {
    let (mut out, mut seen, mut todo) = (HashSet::new(), HashSet::new(), roots);
    let mut first = |v: &Value| v.as_str().is_none_or(|r| seen.insert(r.to_string()));
    while let Some(res) = todo.pop() {
        if !first(res) {
            continue;
        }
        let res = doc.get(res);
        let values = |k: &str| {
            doc.get(&res[k])
                .as_object()
                .into_iter()
                .flat_map(|m| m.values())
        };
        let mut streams: Vec<&Value> = values("/XObject").chain(values("/Pattern")).collect();
        for f in values("/Font").filter(|f| doc.get(f)["/Subtype"] == "/Type3") {
            if first(f) {
                let f = doc.get(f);
                let procs = doc.get(&f["/CharProcs"]).as_object();
                streams.extend(procs.into_iter().flat_map(|m| m.values()));
                todo.push(&f["/Resources"]);
            }
        }
        for x in streams {
            let Some(r) = x.as_str() else { continue };
            let d = obj(doc, r);
            let content =
                d["/Subtype"] == "/Form" || d["/PatternType"] == 1 || d.get("/Subtype").is_none();
            if content && out.insert(r.to_string()) {
                todo.push(&d["/Resources"]);
            }
        }
    }
    out
}

/// References of the content streams to search: what the pages draw (their
/// /Contents and [`drawn`] through their (inherited) /Resources), plus
/// every form XObject that neither an annotation appearance nor a form one
/// draws, drawn or not (fail closed).
fn scanned(doc: &Doc) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut res = Vec::new();
    for p in page_refs(doc) {
        out.extend(
            contents(doc, &p)
                .iter()
                .filter_map(|c| c.as_str().map(String::from)),
        );
        let (mut node, mut depth) = (obj(doc, &p), 0);
        while node.get("/Resources").is_none() && depth < 64 {
            node = doc.get(&node["/Parent"]);
            depth += 1;
        }
        res.push(&node["/Resources"]);
    }
    out.extend(drawn(doc, res));
    let ap = appearances(doc);
    let mut exempt = drawn(doc, ap.iter().map(|r| &obj(doc, r)["/Resources"]).collect());
    exempt.extend(ap);
    let forms = doc
        .objects
        .iter()
        .filter(|(_, o)| o["stream"]["dict"]["/Subtype"] == "/Form");
    out.extend(
        forms
            .map(|(k, _)| k.trim_start_matches("obj:").to_string())
            .filter(|r| !exempt.contains(r)),
    );
    out
}

/// Fails when a string drawn by a page of `doc` holds one of `secret`.
fn scan(doc: &Doc, secret: &Words) -> Result<(), PagesError> {
    for r in scanned(doc) {
        let data = doc.objects.get(&format!("obj:{r}"));
        let data = data
            .and_then(|o| o["stream"]["data"].as_str())
            .and_then(base64);
        if words(&strings(&data.unwrap_or_default()))
            .iter()
            .any(|w| secret.contains(w))
        {
            return Err(PagesError::RedactionLeak);
        }
    }
    Ok(())
}

/// Reads `out` (streams decoded, via a file in `dir`) and fails with
/// `RedactionLeak` when a word of `secret` is in its content.
///
/// # Errors
/// `RedactionLeak`, `Qpdf`, `Sidecar`, `Io`.
pub(super) fn verify(out: &Path, secret: &Words, dir: &Path) -> Result<(), PagesError> {
    if secret.is_empty() {
        return Ok(());
    }
    scan(&Doc::read_data(out, dir)?, secret)
}

#[cfg(test)]
mod tests;
