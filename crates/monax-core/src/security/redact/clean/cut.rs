//! R104: what names a redacted page. A describer of it (a destination
//! array, an action, a struct element, an outline item) goes whole; any
//! other object only loses the entries that name it, so kept pages keep
//! the containers they need (/Annots, /Kids, /XObject maps, forms).
use crate::content::qjson::Doc;
use serde_json::{Map, Value, json};
use std::collections::HashSet;

/// Whether `v` itself (not what it references) names a page of `red`.
pub(super) fn names(v: &Value, red: &HashSet<String>) -> bool {
    match v {
        Value::String(r) => red.contains(r),
        Value::Array(a) => a.iter().any(|x| names(x, red)),
        Value::Object(m) => m.values().any(|x| names(x, red)),
        _ => false,
    }
}

/// Whether `v`, which names a page of `red`, describes it: a destination
/// array starting with it, or a dictionary with /S (actions, struct
/// elements) or /Title (outline items), or typed a struct element or
/// marked-content / object reference.
pub(super) fn describes(v: &Value, red: &HashSet<String>) -> bool {
    const TYPES: [&str; 4] = ["/StructElem", "/MCR", "/OBJR", "/Action"];
    match v {
        Value::Array(a) => a
            .first()
            .and_then(Value::as_str)
            .is_some_and(|r| red.contains(r)),
        Value::Object(m) => {
            m.contains_key("/S")
                || m.contains_key("/Title")
                || m.get("/Type")
                    .and_then(Value::as_str)
                    .is_some_and(|t| TYPES.contains(&t))
        }
        _ => false,
    }
}

/// `v` with every entry that names a page of `red` set to null: the
/// reference itself, or the describer holding it.
pub(super) fn cut(v: &Value, red: &HashSet<String>) -> Value {
    if !names(v, red) {
        return v.clone();
    }
    match v {
        Value::Array(a) if !describes(v, red) => a.iter().map(|x| cut(x, red)).collect(),
        Value::Object(m) if !describes(v, red) => {
            m.iter().map(|(k, x)| (k.clone(), cut(x, red))).collect()
        }
        _ => Value::Null,
    }
}

/// What names a page of `red` in `doc`, outside `stop` and `dead`: each
/// describer of it joins `dead`; every other object (a stream: its
/// dictionary) is returned rewritten by [`cut`], as qpdf JSON updates.
pub(super) fn objects(
    doc: &Doc,
    stop: &HashSet<String>,
    dead: &mut HashSet<String>,
    red: &HashSet<String>,
) -> Map<String, Value> {
    let mut cuts = Map::new();
    for (k, o) in &doc.objects {
        let Some(id) = k.strip_prefix("obj:") else {
            continue;
        };
        let (v, stream) = match &o["value"] {
            Value::Null => (&o["stream"]["dict"], true),
            v => (v, false),
        };
        if stop.contains(id) || dead.contains(id) || !names(v, red) {
            continue;
        }
        if !stream && describes(v, red) {
            dead.insert(id.to_string());
            continue;
        }
        let v = cut(v, red);
        let v = if stream {
            json!({ "stream": { "dict": v } })
        } else {
            json!({ "value": v })
        };
        cuts.insert(k.clone(), v);
    }
    cuts
}
