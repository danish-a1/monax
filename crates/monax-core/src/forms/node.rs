//! A terminal field and its widgets, read from qpdf JSON.
use super::text::{name, text};
use super::{Field, FieldKind};
use crate::content::qjson::Doc;
use crate::pages::Rect;
use serde_json::Value;
use std::collections::HashMap;

/// A widget of a field: its object, page, rect and (checkbox, radio) the
/// raw name of its on state (`/Agreed`) and its export value.
pub(super) struct Widget {
    pub r: String,
    pub page: Option<usize>,
    pub rect: Option<Rect>,
    pub state: Option<String>,
    pub export: Option<String>,
}

/// A terminal field: what [`Field`] lists, its object, its widgets, and
/// what filling it checks.
pub(super) struct Node {
    pub field: Field,
    pub r: String,
    pub widgets: Vec<Widget>,
    pub max_len: Option<usize>,
    pub editable: bool,
    /// Values selected in a list box (the field lists only the first).
    pub selected: usize,
}

/// Inheritable entries (/FT, /Ff, /V, /MaxLen) on the way down.
#[derive(Clone, Default)]
pub(super) struct Inherited<'a> {
    pub name: String,
    pub ft: Option<&'a str>,
    pub ff: u64,
    pub v: Option<&'a Value>,
    pub max_len: Option<usize>,
}

/// The terminal field `d` (object `r`) of `kind`, with widgets `ws`
/// (itself when it has no kids) placed by `pages`.
pub(super) fn node(
    doc: &Doc,
    r: &str,
    d: &Value,
    kind: FieldKind,
    inh: &Inherited,
    ws: &[Value],
    pages: &HashMap<String, usize>,
) -> Node {
    let opt = list(doc, &d["/Opt"]);
    let per_widget = opt.len() == ws.len(); // a button's export values
    let widgets: Vec<Widget> = (ws.iter().enumerate())
        .filter_map(|(i, w)| {
            let wr = w.as_str()?;
            let wd = doc.get(w);
            // A stream /N is one appearance: no states.
            let n = &doc.get(&wd["/AP"])["/N"];
            let n = (!doc.is_stream(n))
                .then(|| doc.get(n).as_object())
                .flatten();
            let state = n.and_then(|n| n.keys().find(|k| *k != "/Off").cloned());
            let export = match per_widget {
                true => text(&opt[i]),
                false => state.as_deref().map(name),
            };
            Some(Widget {
                r: wr.to_string(),
                page: pages.get(wr).copied(),
                rect: rect(doc.get(&wd["/Rect"])),
                state: state.filter(|_| matches!(kind, FieldKind::Checkbox | FieldKind::Radio)),
                export,
            })
        })
        .collect();
    let v = inh.v.map(|v| doc.get(v));
    let selected = v.and_then(Value::as_array).map_or(0, Vec::len);
    let (value, options) = match kind {
        FieldKind::Checkbox | FieldKind::Radio => {
            let on = v.and_then(Value::as_str).filter(|s| *s != "/Off");
            let w = on.and_then(|s| widgets.iter().find(|w| w.state.as_deref() == Some(s)));
            let value = match (on, w) {
                (None, _) => "Off".to_string(),
                (Some(_), Some(w)) => w.export.clone().unwrap_or_default(),
                (Some(s), None) => name(s),
            };
            let mut opts: Vec<String> = Vec::new();
            let exports = widgets.iter().filter(|w| w.state.is_some());
            for e in exports.filter_map(|w| w.export.clone()) {
                if !opts.contains(&e) {
                    opts.push(e);
                }
            }
            (value, opts)
        }
        FieldKind::Combo | FieldKind::List => {
            let first = v.and_then(|v| v.as_array().map_or(Some(v), |a| a.first()));
            let opts = opt
                .iter()
                .filter_map(|o| text(o.as_array().map_or(o, |a| &a[0])));
            (first.and_then(text).unwrap_or_default(), opts.collect())
        }
        FieldKind::Text => (v.and_then(text).unwrap_or_default(), Vec::new()),
        FieldKind::Button | FieldKind::Signature => (String::new(), Vec::new()),
    };
    let first = widgets.iter().find(|w| w.page.is_some());
    Node {
        field: Field {
            name: inh.name.clone(),
            kind,
            page: first.and_then(|w| w.page),
            rect: first.and_then(|w| w.rect),
            value,
            options,
            read_only: inh.ff & 1 != 0,
        },
        r: r.to_string(),
        widgets,
        max_len: inh.max_len,
        editable: inh.ff & (1 << 18) != 0,
        selected,
    }
}

/// The array `v` is or references; empty otherwise.
pub(super) fn list<'a>(doc: &'a Doc, v: &'a Value) -> &'a [Value] {
    doc.get(v).as_array().map_or(&[], Vec::as_slice)
}

/// A /Rect as [`Rect`], corners in either order.
fn rect(v: &Value) -> Option<Rect> {
    let n: Vec<f32> = v
        .as_array()?
        .iter()
        .filter_map(|x| x.as_f64())
        .map(|x| x as f32)
        .collect();
    let [a, b, c, d] = n[..] else { return None };
    Some(Rect {
        left: a.min(c),
        bottom: b.min(d),
        right: a.max(c),
        top: b.max(d),
    })
}
