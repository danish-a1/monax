//! The field walk behind [`super::check`] and [`super::original`].
use crate::content::qjson::Doc;
use crate::pdfium::PdfiumError;
use serde_json::Value;
use std::collections::HashSet;

/// Walks a document already read: the catalog's /AcroForm /Fields, then
/// every page's /Annots widgets (pdfium loads fields from both, R116),
/// through each field's /Kids (arrays read by their own elements),
/// iteratively, in time linear in the objects walked. A widget already
/// reached from /Fields is not walked again; one first reached from
/// /Annots may later be reached once as a kid. Each field's and widget's
/// /Parent chain is followed at most [`DEPTH`] steps. A null (or dangling)
/// entry is skipped when pdfium reads the same rewrite (R118); with
/// `strict` (pdfium reads the original, R119) it fails closed.
///
/// # Errors
/// `FieldRevisit` when the walk reaches an object twice: a field, an
/// indirect /Kids array (direct fields sharing one), or a widget listed in
/// /Annots twice across pages. `FieldDepth` when a /Parent chain is longer
/// than [`DEPTH`] (or a cycle). With `strict`, `FieldCheck` when a form,
/// field list, field, page /Annots or /Annots element reads as null though
/// it is named: qpdf writes a reference it cannot resolve as null (a
/// generation mismatch, an object an update freed), which pdfium may still
/// resolve by number.
pub(super) fn check_doc(doc: &Doc, strict: bool) -> Result<(), PdfiumError> {
    let root = doc.get(&doc.trailer()["/Root"]);
    let form = doc.get(&root["/AcroForm"]);
    let mut w = Walk {
        doc,
        strict,
        seen: HashSet::new(),
        roots: HashSet::new(),
    };
    w.lost(root, "/AcroForm")?;
    w.lost(form, "/Fields")?;
    w.walk(&form["/Fields"])?;
    let pages: Vec<Value> = doc.page_tree().0.into_iter().map(Value::from).collect();
    let mut listed = HashSet::new();
    for p in &pages {
        let page = doc.get(p);
        // TODO: qpdf drops a dictionary key whose reference it cannot
        // resolve, which pdfium may resolve by number: `lost` sees only a
        // key qpdf kept. Harmless on qpdf's rewrite; open for `strict` (R119).
        w.lost(page, "/Annots")?;
        for a in doc
            .get(&page["/Annots"])
            .as_array()
            .map_or(&[][..], Vec::as_slice)
        {
            // An indirect /Subtype is compared raw: pdfium loads no field
            // from it (measured, R117).
            match doc.get(a) {
                Value::Null if strict => return Err(unknown()),
                d if d["/Subtype"] != "/Widget" => continue,
                _ => {}
            }
            w.shallow(a)?;
            match as_ref(a) {
                // pdfium walks a widget once per listing.
                Some(r) if !listed.insert(r) => return Err(PdfiumError::FieldRevisit),
                Some(r) if w.seen.contains(r) => {}
                Some(r) => {
                    w.roots.insert(r);
                    w.seen.insert(r);
                    w.kids(doc.get(a))?;
                }
                None => w.kids(a)?,
            }
        }
    }
    Ok(())
}

/// pdfium nests fields at most 32 deep, but joins a field's whole /Parent
/// chain (no cap) into its name, once per field and widget: 5000 widgets
/// under a 5000-field chain take about 30 s. Cut here: 33 steps a field.
const DEPTH: usize = 32;

fn as_ref(v: &Value) -> Option<&str> {
    v.as_str().filter(|r| r.ends_with(" R"))
}

fn unknown() -> PdfiumError {
    PdfiumError::FieldCheck("a field reference does not resolve".into())
}

/// The field walk: `seen` holds every reference reached, `roots` the
/// widgets entered from /Annots that no /Kids has reached yet.
struct Walk<'a> {
    doc: &'a Doc,
    strict: bool,
    seen: HashSet<&'a str>,
    roots: HashSet<&'a str>,
}

impl<'a> Walk<'a> {
    /// Walks the field list `list` and everything below it.
    fn walk(&mut self, list: &'a Value) -> Result<(), PdfiumError> {
        let mut todo = vec![list];
        while let Some(list) = todo.pop() {
            self.reach(list)?; // an indirect array
            for v in self.doc.get(list).as_array().map_or(&[][..], Vec::as_slice) {
                if self.reach(v)? {
                    let f = self.doc.get(v);
                    if self.strict && f.is_null() {
                        return Err(unknown());
                    }
                    self.lost(f, "/Kids")?;
                    self.shallow(v)?;
                    todo.push(&f["/Kids"]);
                }
            }
        }
        Ok(())
    }

    /// Walks the /Kids of field `f`.
    fn kids(&mut self, f: &'a Value) -> Result<(), PdfiumError> {
        self.lost(f, "/Kids")?;
        self.walk(&f["/Kids"])
    }

    /// With `strict`, fails when `d` names `k` (the key is there) but it
    /// reads as null.
    fn lost(&self, d: &Value, k: &str) -> Result<(), PdfiumError> {
        match d.get(k) {
            Some(v) if self.strict && self.doc.get(v).is_null() => Err(unknown()),
            _ => Ok(()),
        }
    }

    /// Fails unless field `f` has at most [`DEPTH`] /Parent ancestors.
    fn shallow(&self, f: &Value) -> Result<(), PdfiumError> {
        let mut p = &self.doc.get(f)["/Parent"];
        for _ in 0..=DEPTH {
            if p.is_null() {
                return Ok(());
            }
            p = &self.doc.get(p)["/Parent"];
        }
        Err(PdfiumError::FieldDepth)
    }

    /// Marks `v` reached; false when it is a widget already walked from
    /// /Annots (now reached once, as a kid).
    fn reach(&mut self, v: &'a Value) -> Result<bool, PdfiumError> {
        match as_ref(v) {
            Some(r) if self.roots.remove(r) => Ok(false),
            Some(r) if !self.seen.insert(r) => Err(PdfiumError::FieldRevisit),
            _ => Ok(true),
        }
    }
}
