//! New field values as qpdf object updates: each field's /V, and its
//! widgets' /AS (checkbox, radio) or the removal of their stale appearance
//! (text, combo, list), which pdfium then draws anew.
use super::FieldKind as K;
use super::node::Node;
use crate::content::qjson::Doc;
use crate::pages::PagesError;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// The objects to rewrite, and the pages whose widgets pdfium must draw.
pub(super) struct Plan<'a> {
    doc: &'a Doc,
    edits: BTreeMap<String, Map<String, Value>>,
    pub pages: BTreeSet<usize>,
}

fn bad(s: String) -> PagesError {
    PagesError::InvalidInput(s)
}

impl<'a> Plan<'a> {
    pub fn new(doc: &'a Doc) -> Self {
        Self {
            doc,
            edits: BTreeMap::new(),
            pages: BTreeSet::new(),
        }
    }

    /// The pending dictionary of object `r` ("N G R"), first its current one.
    fn obj(&mut self, r: &str) -> &mut Map<String, Value> {
        let doc = self.doc;
        self.edits.entry(r.to_string()).or_insert_with(|| {
            let r = Value::from(r);
            doc.get(&r).as_object().cloned().unwrap_or_default()
        })
    }

    /// Sets field `n` to `value` (see [`super::FieldValue`]).
    ///
    /// # Errors
    /// `InvalidInput` naming the field and the reason: read-only, a push
    /// button or signature, no widget on a page, longer than /MaxLen, not
    /// one of its options or export values.
    pub fn set(&mut self, n: &Node, value: &str) -> Result<(), PagesError> {
        let f = &n.field;
        let why = match f.kind {
            _ if f.read_only => Some("is read-only"),
            K::Button => Some("is a push button, which holds no value"),
            K::Signature => Some("is a signature field, which cannot be filled"),
            _ if f.page.is_none() => Some("has no widget on a page, so its value would not show"),
            _ => None,
        };
        if let Some(why) = why {
            return Err(bad(format!("field {:?} {why}", f.name)));
        }
        match f.kind {
            K::Checkbox | K::Radio => self.check(n, value)?,
            _ => self.text(n, value)?,
        }
        self.pages.extend(n.widgets.iter().filter_map(|w| w.page));
        Ok(())
    }

    fn text(&mut self, n: &Node, value: &str) -> Result<(), PagesError> {
        let f = &n.field;
        if let Some(max) = n.max_len.filter(|&m| value.chars().count() > m) {
            let why = format!(
                "the value for field {:?} is longer than {max} characters",
                f.name
            );
            return Err(bad(why));
        }
        let at = f.options.iter().position(|o| o == value);
        if f.kind != K::Text && at.is_none() && !(f.kind == K::Combo && n.editable) {
            return Err(bad(format!(
                "the value for field {:?} is not one of its options ({})",
                f.name,
                f.options.join(", ")
            )));
        }
        let d = self.obj(&n.r);
        d.insert("/V".into(), json!(format!("u:{value}")));
        d.remove("/RV"); // a rich text value would contradict it
        match at.filter(|_| f.kind != K::Text) {
            Some(i) => d.insert("/I".into(), json!([i])),
            None => d.remove("/I"),
        };
        for w in &n.widgets {
            self.obj(&w.r).remove("/AP");
        }
        Ok(())
    }

    fn check(&mut self, n: &Node, value: &str) -> Result<(), PagesError> {
        let f = &n.field;
        let box_ = f.kind == K::Checkbox;
        let on = n.widgets.iter().filter(|w| w.state.is_some());
        let target = match value {
            "Off" => None,
            "false" if box_ => None,
            "true" if box_ => Some(on.clone().next()),
            v => Some(on.clone().find(|w| w.export.as_deref() == Some(v))),
        };
        let target = match target {
            None => None,
            Some(Some(w)) => w.state.clone(),
            Some(None) => {
                return Err(bad(format!(
                    "the value for field {:?} is not one of its values ({}Off)",
                    f.name,
                    f.options
                        .iter()
                        .map(|o| format!("{o}, "))
                        .collect::<String>()
                )));
            }
        };
        let off = Value::from("/Off");
        let v = target.as_ref().map_or(off.clone(), |s| json!(s));
        self.obj(&n.r).insert("/V".into(), v);
        for w in &n.widgets {
            let shown = target.is_some() && w.state == target;
            let s = if shown { json!(w.state) } else { off.clone() };
            self.obj(&w.r).insert("/AS".into(), s);
        }
        Ok(())
    }

    /// Drops the form's /XFA (and the catalog's /NeedsRendering), so the
    /// standard fields are what every reader shows; whether it had one.
    pub fn drop_xfa(&mut self) -> bool {
        let doc = self.doc;
        let at = &doc.get(&doc.trailer()["/Root"])["/AcroForm"];
        let Some(root) = doc.trailer()["/Root"].as_str() else {
            return false;
        };
        if doc.get(at).get("/XFA").is_none() {
            return false;
        }
        match at.as_str() {
            Some(r) => self.obj(r).remove("/XFA"),
            None => (self.obj(root).get_mut("/AcroForm"))
                .and_then(Value::as_object_mut)
                .and_then(|f| f.remove("/XFA")),
        };
        self.obj(root).remove("/NeedsRendering");
        true
    }

    /// The edits as qpdf JSON object updates.
    pub fn updates(self) -> Map<String, Value> {
        let obj = |(r, d)| (format!("obj:{r}"), json!({ "value": d }));
        self.edits.into_iter().map(obj).collect()
    }
}
