//! R101 / R103 / R108: the objects a redaction makes unreachable -
//! everything reachable from a redacted page's dictionary that no kept
//! page draws with, whatever document-level entry also names it (no document-level root
//! keeps anything: the output is built from the kept pages alone).
use super::obj;
use super::render::{K, render};
use crate::content::qjson::Doc;
use serde_json::Value;
use std::collections::HashSet;

/// Objects reachable from `roots` through every key, not entering `stop`.
fn reach<'a>(doc: &'a Doc, roots: Vec<&'a Value>, stop: &HashSet<String>) -> HashSet<String> {
    let (mut seen, mut todo) = (HashSet::new(), roots);
    while let Some(v) = todo.pop() {
        match v {
            Value::String(r) if r.ends_with(" R") => {
                if !stop.contains(r) && seen.insert(r.clone()) {
                    todo.push(obj(doc, r));
                }
            }
            Value::Array(a) => todo.extend(a),
            Value::Object(m) => todo.extend(m.values()),
            _ => {}
        }
    }
    seen
}

/// The dictionary of page `p` plus the /Resources it inherits, as roots.
fn page_roots<'a>(doc: &'a Doc, p: &str) -> Vec<&'a Value> {
    let page = obj(doc, p);
    let (mut node, mut depth) = (page, 0);
    while node.get("/Resources").is_none() && node.get("/Parent").is_some() && depth < 64 {
        node = doc.get(&node["/Parent"]);
        depth += 1;
    }
    vec![page, &node["/Resources"]]
}

/// [`page_roots`] of pages, with their roles: a page, then its resources.
fn kinds(v: Vec<&Value>) -> Vec<(&Value, K)> {
    let k = |i: usize| if i.is_multiple_of(2) { K::Page } else { K::Res };
    v.into_iter().enumerate().map(|(i, x)| (x, k(i))).collect()
}

/// What a redaction keeps and drops (see [`owned`]).
pub(super) struct Reach {
    /// Reachable from a redacted page and not drawn by a kept page.
    pub owned: HashSet<String>,
    /// Reachable from a kept page through any key.
    pub kept: HashSet<String>,
    /// Groups kept pages draw with, and groups redacted pages draw with.
    pub groups: HashSet<String>,
    pub red_groups: HashSet<String>,
}

/// What only the `red` pages own: everything reachable from them that no
/// kept page draws with (R108: [`render`]; a kept page's other keys never
/// keep anything); what kept pages reach and the groups pages draw with.
/// No walk enters `stop` (pages, page tree nodes, the catalog: qpdf does
/// not copy them with a page). `dead` (the annotations only `red` pages
/// list) is never entered from a kept page, so a kept reply or action
/// pointing at one does not keep what it owns.
pub(super) fn owned(
    doc: &Doc,
    pages: &[String],
    red: &[usize],
    stop: &HashSet<String>,
    dead: &HashSet<String>,
) -> Reach {
    let roots = |keep: bool| -> Vec<&Value> {
        let idx = (0..pages.len()).filter(|i| red.contains(i) != keep);
        idx.flat_map(|i| page_roots(doc, &pages[i])).collect()
    };
    let shut = stop | dead;
    let mine = reach(doc, roots(false), stop);
    let (drawn, groups) = render(doc, kinds(roots(true)), &shut);
    let (_, red_groups) = render(doc, kinds(roots(false)), stop);
    Reach {
        owned: &mine - &drawn,
        kept: reach(doc, roots(true), &shut),
        groups,
        red_groups,
    }
}
