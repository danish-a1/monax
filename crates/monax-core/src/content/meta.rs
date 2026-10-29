//! set_metadata: the document information dictionary.
use super::PagesError;
use super::qjson::Doc;
use super::rewrite::qpdf_write;
use crate::output::private_dir;
use crate::pages::open;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

/// Document information entries: `None` leaves an entry as it is, an empty
/// string removes it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Metadata {
    /// Document title.
    pub title: Option<String>,
    /// Author.
    pub author: Option<String>,
    /// Subject.
    pub subject: Option<String>,
    /// Keywords.
    pub keywords: Option<String>,
    /// Application that created the original document.
    pub creator: Option<String>,
    /// Application that produced the PDF.
    pub producer: Option<String>,
}

impl Metadata {
    fn entries(&self) -> [(&'static str, &Option<String>); 6] {
        [
            ("/Title", &self.title),
            ("/Author", &self.author),
            ("/Subject", &self.subject),
            ("/Keywords", &self.keywords),
            ("/Creator", &self.creator),
            ("/Producer", &self.producer),
        ]
    }
}

/// Writes `input` to `out` with the Info dictionary entries of `meta` set
/// (or removed); other entries and the page content are unchanged. XMP
/// metadata, when present, is left as it is. Returns the path written.
///
/// # Errors
/// `InvalidInput` (bad paths), `EncryptedInput`, `Damaged`, `Qpdf`,
/// `Sidecar`, `Pdfium`, `Io`.
pub fn set_metadata(input: &Path, out: &Path, meta: Metadata) -> Result<PathBuf, PagesError> {
    // TODO: XMP /Metadata can disagree with the new Info entries; sync it if
    // viewers that prefer XMP need it.
    open(input)?;
    let dir = private_dir()?; // document data
    let doc = Doc::read(input, dir.path())?;
    let at = &doc.trailer()["/Info"];
    let mut info = doc.get(at).as_object().cloned().unwrap_or_default();
    for (key, v) in meta.entries() {
        match v.as_deref() {
            None => {}
            Some("") => {
                info.remove(key);
            }
            Some(s) => {
                info.insert(key.into(), json!(format!("u:{s}")));
            }
        }
    }
    let mut updates = Map::new();
    let r = match at.as_str() {
        Some(r) => r.to_string(),
        None => {
            // No Info object yet (or a direct one): add one, point the trailer at it.
            let r = format!("{} 0 R", doc.new_id());
            let mut trailer = doc.trailer().clone();
            trailer["/Info"] = Value::String(r.clone());
            updates.insert("trailer".into(), json!({ "value": trailer }));
            r
        }
    };
    updates.insert(format!("obj:{r}"), json!({ "value": info }));
    let arg = doc.update_arg(updates, dir.path())?;
    qpdf_write(input, input, vec![arg], out)
}
