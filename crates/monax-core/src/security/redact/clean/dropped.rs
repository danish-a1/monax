//! The document-level features a redacted output loses: its catalog is cut
//! to its pages, form and kept layers (R103, R104), so the catalog's other
//! entries, the trailer's /Info and the form's /XFA are not carried. Named
//! for the job notes.
use crate::content::qjson::Doc;
use serde_json::Value;

/// Catalog keys with a label below, plus those that are rebuilt (pages,
/// form fields), versions, and tags (noted on their own). Any other key is
/// named as it is.
const KNOWN: [&str; 20] = [
    "/Outlines",
    "/Dests",
    "/Names",
    "/AF",
    "/OCProperties",
    "/PageLabels",
    "/Metadata",
    "/OpenAction",
    "/AA",
    "/ViewerPreferences",
    "/PageMode",
    "/PageLayout",
    "/Threads",
    "/Type",
    "/Pages",
    "/AcroForm",
    "/Version",
    "/Extensions",
    "/StructTreeRoot",
    "/MarkInfo",
];

/// /Names trees with a label below; others are named `/Names /<key>`.
const NAMED: [&str; 3] = ["/Dests", "/JavaScript", "/EmbeddedFiles"];

/// Labels of what `cat` (the catalog), the trailer and `form` (the form
/// dictionary after pruning) hold that the output drops, each once.
pub(super) fn dropped(doc: &Doc, cat: &Value, form: &Value) -> Vec<String> {
    let names = doc
        .get(&cat["/Names"])
        .as_object()
        .cloned()
        .unwrap_or_default();
    let cat = cat.as_object().cloned().unwrap_or_default();
    let has = |k: &str| cat.contains_key(k);
    let named = |k: &str| names.contains_key(k);
    // An outline tree without items holds no bookmark.
    let outlines = cat
        .get("/Outlines")
        .is_some_and(|o| !doc.get(o)["/First"].is_null());
    let found = [
        (outlines, "bookmarks"),
        (has("/Dests") || named("/Dests"), "named destinations"),
        (named("/JavaScript"), "document JavaScript"),
        (named("/EmbeddedFiles") || has("/AF"), "attachments"),
        (has("/PageLabels"), "page labels"),
        (
            has("/Metadata") || doc.trailer().get("/Info").is_some(),
            "metadata",
        ),
        (has("/OpenAction"), "the open action"),
        (has("/AA"), "document actions"),
        (
            ["/ViewerPreferences", "/PageMode", "/PageLayout"]
                .iter()
                .any(|k| has(k)),
            "viewer settings",
        ),
        (has("/Threads"), "article threads"),
        (form.get("/XFA").is_some(), "XFA form data"),
    ];
    let mut out: Vec<String> = found
        .iter()
        .filter(|(f, _)| *f)
        .map(|(_, l)| l.to_string())
        .collect();
    let other = cat.keys().filter(|k| !KNOWN.contains(&k.as_str()));
    out.extend(other.cloned());
    let trees = names.keys().filter(|k| !NAMED.contains(&k.as_str()));
    out.extend(trees.map(|k| format!("/Names {k}")));
    out
}
