//! R104: optional content kept pages use stays, with its default state,
//! so content a kept page hides stays hidden; groups no kept page uses go.
//! The properties are built from scratch out of kept group references, so
//! nothing else in the input's (labels, configurations, other keys or the
//! objects they reference) can reach the output. R106: every kept group
//! dictionary is cut to its type, name and intent, so no layer rescues
//! anything either.
use super::ocg;
use crate::content::qjson::Doc;
use serde_json::{Map, Value, json};
use std::collections::HashSet;

/// The /OCProperties of catalog `root` rebuilt for the `kept` groups
/// (those kept pages draw with; `red`: those redacted pages draw with):
/// `/OCGs` those groups, `/D` the input default configuration's /BaseState
/// (/OFF for "OFF", a name or a string, as pdfium reads it; /ON and
/// /Unchanged names as they are) and its /ON, /OFF and /Order (flattened, labels
/// dropped) cut to them, /OCGs, /ON and /OFF read by their own elements; every other key goes, and so does anything else
/// /OCGs lists. Each kept group (listed or not) is rewritten into `up` by
/// [`ocg::group`]; one its /Usage forces off is listed (when it was not)
/// and moved to /OFF, one it forces on moved to /ON, so it looks the same
/// without /Usage. `None` when no group is left. Also whether a listed
/// group went (one typed /OCG or drawn with by a redacted page). A null
/// or dangling /OCGs entry is no group, as in pdfium (R112: input qpdf
/// warns on never gets here).
pub(super) fn layers(
    doc: &Doc,
    root: &str,
    (kept, red): (&HashSet<String>, &HashSet<String>),
    up: &mut Map<String, Value>,
) -> (Option<Value>, bool) {
    let ocp = doc.get(&super::obj(doc, root)["/OCProperties"]);
    let all = groups(doc, &ocp["/OCGs"], &|_| true, false);
    let typed = |r: &str| ocg::text(doc, &super::obj(doc, r)["/Type"]) == Some("OCG");
    let gone = all
        .iter()
        .any(|g| !kept.contains(g) && (red.contains(g) || typed(g)));
    let listed: HashSet<&str> = all.iter().map(String::as_str).collect();
    let mut ocgs: Vec<String> = all.iter().filter(|g| kept.contains(*g)).cloned().collect();
    let mut strays: Vec<&String> = kept
        .iter()
        .filter(|r| !listed.contains(r.as_str()))
        .collect();
    strays.sort(); // the same output every run
    let (mut on, mut off) = (Vec::new(), Vec::new());
    for g in ocgs.iter().chain(strays.iter().copied()) {
        let Some((v, state)) = ocg::group(doc, g) else {
            continue;
        };
        up.insert(format!("obj:{g}"), json!({ "value": v }));
        match state {
            Some(true) => on.push(g.clone()),
            Some(false) => off.push(g.clone()),
            None => {}
        }
    }
    ocgs.extend(off.iter().filter(|g| !listed.contains(g.as_str())).cloned());
    if ocgs.is_empty() {
        return (None, gone);
    }
    let set: HashSet<&str> = ocgs.iter().map(String::as_str).collect();
    let d = doc.get(&ocp["/D"]);
    let mut out = Map::new();
    if ocg::text(doc, &d["/BaseState"]) == Some("OFF") {
        out.insert("/BaseState".into(), "/OFF".into());
    } else if let Some(b) = doc.get(&d["/BaseState"]).as_str()
        && ["/ON", "/Unchanged"].contains(&b)
    {
        out.insert("/BaseState".into(), b.into());
    }
    for (k, forced, other) in [
        ("/ON", &on, &off),
        ("/OFF", &off, &on),
        ("/Order", &vec![], &vec![]),
    ] {
        let not: HashSet<&str> = other.iter().map(String::as_str).collect();
        let keep = |r: &str| set.contains(r) && !not.contains(r);
        let mut v = groups(doc, &d[k], &keep, k == "/Order");
        let have: HashSet<String> = v.iter().cloned().collect();
        let more = forced
            .iter()
            .filter(|g| set.contains(g.as_str()) && !have.contains(*g));
        v.extend(more.cloned().collect::<Vec<_>>());
        if !v.is_empty() {
            out.insert(k.into(), v.into());
        }
    }
    (Some(json!({ "/OCGs": ocgs, "/D": out })), gone)
}

/// The references `keep` accepts among the elements of array `v` (direct
/// or indirect), in order and without repeats; with `nest`, also in the
/// arrays it nests (direct or indirect, each entered once: /Order). pdfium
/// reads /OCGs, /ON and /OFF by their own elements only, so a group in a
/// sub-array of one is not in it. Iterative: a chain of indirect arrays is
/// as deep as the input makes it.
fn groups(doc: &Doc, v: &Value, keep: &dyn Fn(&str) -> bool, nest: bool) -> Vec<String> {
    let (mut out, mut seen, mut todo) = (Vec::new(), HashSet::new(), vec![(v, true)]);
    while let Some((v, top)) = todo.pop() {
        match v {
            Value::String(r) if r.ends_with(" R") => {
                if !seen.insert(r) {
                    continue;
                }
                match doc.get(v) {
                    a @ Value::Array(_) if top || nest => todo.push((a, top)),
                    Value::Array(_) => {}
                    _ if keep(r) => out.push(r.clone()),
                    _ => {}
                }
            }
            Value::Array(a) if top || nest => todo.extend(a.iter().rev().map(|x| (x, false))),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
#[path = "layers_tests.rs"]
mod tests;
