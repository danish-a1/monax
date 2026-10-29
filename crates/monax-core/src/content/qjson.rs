//! qpdf JSON v2: read a document's objects, apply object updates; qpdf's
//! rewrite of a document, refused as [`Doc::read_intact`] refuses.
mod run;
mod stream;

use super::PagesError;
use crate::output::private_file;
use crate::pdfium::PdfiumError;
pub(crate) use run::rewrite_intact;
use run::run;
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::ffi::OsString;
use std::path::Path;
pub(crate) use stream::stream;

/// A document's objects as qpdf JSON v2 (stream data not included). Values
/// nest at most 128 deep (serde_json's parse limit), which bounds every
/// recursive walk over one; walks through references iterate.
pub(crate) struct Doc {
    header: Value,
    /// `"obj:N G R"` and `"trailer"` -> `{"value": ..}` or `{"stream": {"dict": ..}}`.
    pub objects: Map<String, Value>,
}

fn unreadable(e: impl std::fmt::Display) -> PagesError {
    PagesError::Qpdf(format!("unreadable qpdf JSON: {e}"))
}

impl Doc {
    /// A document of `objects` alone (unit tests).
    #[cfg(test)]
    pub fn of(objects: Value) -> Self {
        let objects = objects.as_object().cloned().unwrap_or_default();
        Self {
            header: json!({ "maxobjectid": objects.len() }),
            objects,
        }
    }

    /// Reads `input`'s objects with `qpdf --json-output` via a file in `dir`.
    pub fn read(input: &Path, dir: &Path) -> Result<Self, PagesError> {
        Self::read_opened(input, dir, None)
    }

    /// [`Doc::read`] of `input` opened with `password` when given (handed
    /// to qpdf in a 0600 file, never argv).
    pub fn read_opened(
        input: &Path,
        dir: &Path,
        password: Option<&str>,
    ) -> Result<Self, PagesError> {
        Self::read_with(input, dir, &["--json-stream-data=none"], false, password)
    }

    /// [`Doc::read_opened`] for a redaction or a form check:
    /// `DamagedRepairFirst` when qpdf exits with warnings (3) or errors (2)
    /// reading `input` (R112; pdfium loaded it, so the two readers may
    /// disagree, e.g. on a rebuilt xref, and qpdf may have dropped what
    /// pdfium draws or hides); `DamagedNesting` when the JSON nests past
    /// the reader's limit.
    pub fn read_intact(
        input: &Path,
        dir: &Path,
        password: Option<&str>,
    ) -> Result<Self, PagesError> {
        Self::read_with(input, dir, &["--json-stream-data=none"], true, password)
    }

    /// [`Doc::read_intact`] for an op that writes qpdf's reading of
    /// `input` (not a redaction): a file qpdf must repair to read is
    /// `Pdfium(DamagedRepairFirst)`, "damaged file: repair it first".
    pub fn read_unrepaired(input: &Path, dir: &Path) -> Result<Self, PagesError> {
        Self::read_intact(input, dir, None).map_err(|e| match e {
            PagesError::DamagedRepairFirst => PdfiumError::DamagedRepairFirst.into(),
            e => e,
        })
    }

    /// [`Doc::read`] with every stream's data decoded (generalized
    /// filters), base64 in `stream.data`.
    pub fn read_data(input: &Path, dir: &Path) -> Result<Self, PagesError> {
        let flags = ["--json-stream-data=inline", "--decode-level=generalized"];
        Self::read_with(input, dir, &flags, false, None)
    }

    fn read_with(
        input: &Path,
        dir: &Path,
        flags: &[&str],
        intact: bool,
        password: Option<&str>,
    ) -> Result<Self, PagesError> {
        let f = dir.join("objects.json");
        private_file(&f)?;
        let flags = [&["--json-output=2"], flags].concat();
        run(input, password, &flags, &f, intact)?;
        let mut v: Value = serde_json::from_slice(&std::fs::read(&f)?).map_err(|e| {
            let deep = e.to_string().starts_with("recursion limit exceeded");
            if intact && deep {
                PagesError::DamagedNesting
            } else {
                unreadable(e)
            }
        })?;
        std::fs::remove_file(&f)?;
        let mut q = match v["qpdf"].take() {
            Value::Array(q) if q.len() == 2 => q,
            _ => return Err(unreadable("no qpdf array")),
        };
        let objects = match q.pop() {
            Some(Value::Object(m)) => m,
            _ => return Err(unreadable("no objects")),
        };
        let header = q.pop().unwrap_or_default();
        Ok(Self { header, objects })
    }

    /// `v`, or the value of the object it references (`"N G R"`); a
    /// stream's dictionary. `Null` for a dangling reference.
    pub fn get<'a>(&'a self, v: &'a Value) -> &'a Value {
        match v.as_str().filter(|r| r.ends_with(" R")) {
            Some(r) => {
                let o = &self.objects.get(&format!("obj:{r}"));
                let o = o.map_or(&Value::Null, |o| o);
                match &o["value"] {
                    Value::Null => &o["stream"]["dict"],
                    v => v,
                }
            }
            None => v,
        }
    }

    /// Whether `v` references a stream (whose [`Doc::get`] is its dictionary).
    pub fn is_stream(&self, v: &Value) -> bool {
        let o = v
            .as_str()
            .and_then(|r| self.objects.get(&format!("obj:{r}")));
        o.is_some_and(|o| o.get("stream").is_some())
    }

    /// Page object references (`"N G R"`) in page order, and the page tree
    /// nodes above them (and their indirect /Kids arrays). Iterative: the
    /// tree is as deep as the input makes it.
    pub fn page_tree(&self) -> (Vec<String>, HashSet<String>) {
        let (mut out, mut seen) = (Vec::new(), HashSet::new());
        let mut todo = vec![&self.get(&self.trailer()["/Root"])["/Pages"]];
        while let Some(node) = todo.pop() {
            let Some(r) = node.as_str() else { continue };
            if !seen.insert(r.to_string()) {
                continue; // a cycle
            }
            let kids = &self.get(node)["/Kids"];
            if let Some(k) = kids.as_str() {
                seen.insert(k.to_string()); // an indirect /Kids array
            }
            match self.get(kids) {
                Value::Array(kids) => todo.extend(kids.iter().rev()),
                _ => out.push(r.to_string()),
            }
        }
        for p in &out {
            seen.remove(p);
        }
        (out, seen)
    }

    /// The trailer dictionary.
    pub fn trailer(&self) -> &Value {
        &self.objects["trailer"]["value"]
    }

    /// An object number no existing object uses.
    pub fn new_id(&self) -> u64 {
        self.header["maxobjectid"].as_u64().unwrap_or(0) + 1
    }

    /// Writes `updates` (same shape as [`Doc::objects`]) to a file in `dir`
    /// and returns the qpdf argument that applies it.
    pub fn update_arg(
        &self,
        updates: Map<String, Value>,
        dir: &Path,
    ) -> Result<OsString, PagesError> {
        let f = dir.join("update.json");
        private_file(&f)?;
        std::fs::write(&f, json!({ "qpdf": [self.header, updates] }).to_string())?;
        let mut arg = OsString::from("--update-from-json=");
        arg.push(f);
        Ok(arg)
    }
}
