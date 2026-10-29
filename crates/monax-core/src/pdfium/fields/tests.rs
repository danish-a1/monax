use super::*;
use serde_json::{Map, Value, json};

mod strict;

/// A document whose catalog's /AcroForm is `form`, with one page (3)
/// whose /Annots is `annots` (none when null), and `objs` (`N` -> value of `N 0 R`).
pub(super) fn doc(form: Value, annots: Value, objs: &[(u32, Value)]) -> Doc {
    let mut o = Map::new();
    o.insert("trailer".into(), json!({ "value": { "/Root": "1 0 R" } }));
    let cat = json!({ "value": { "/AcroForm": form, "/Pages": "2 0 R" } });
    o.insert("obj:1 0 R".into(), cat);
    o.insert(
        "obj:2 0 R".into(),
        json!({ "value": { "/Kids": ["3 0 R"] } }),
    );
    let page = if annots.is_null() {
        json!({})
    } else {
        json!({ "/Annots": annots })
    };
    o.insert("obj:3 0 R".into(), json!({ "value": page }));
    for (n, v) in objs {
        o.insert(format!("obj:{n} 0 R"), json!({ "value": v }));
    }
    Doc::of(Value::Object(o))
}

fn leaf() -> Value {
    json!({ "/T": "(w)" })
}

/// The message of `check_doc`'s error, or "ok".
fn walk(form: Value, objs: &[(u32, Value)]) -> String {
    walk_annots(form, Value::Null, objs)
}

/// [`walk`] with page 3's /Annots.
fn walk_annots(form: Value, annots: Value, objs: &[(u32, Value)]) -> String {
    let d = doc(form, annots, objs);
    walk::check_doc(&d, false).map_or_else(|e| e.to_string(), |_| "ok".into())
}

pub(super) fn widget(kids: Value) -> Value {
    json!({ "/Subtype": "/Widget", "/T": "(w)", "/Kids": kids })
}

#[test]
fn a_tree_passes_and_a_field_reached_twice_fails() {
    let twice = "damaged file: form field tree reaches a field twice";
    let tree = [
        (5, json!({ "/Kids": ["6 0 R", "7 0 R"] })),
        (6, leaf()),
        (7, leaf()),
    ];
    assert_eq!(walk(json!({ "/Fields": ["5 0 R"] }), &tree), "ok");
    assert_eq!(walk(json!({}), &[]), "ok", "no fields");
    assert_eq!(walk(Value::Null, &[]), "ok", "a form named but null");
    let dag = [(5, json!({ "/Kids": ["6 0 R", "6 0 R"] })), (6, leaf())];
    assert_eq!(walk(json!({ "/Fields": ["5 0 R"] }), &dag), twice);
    let listed = json!({ "/Fields": ["6 0 R", "6 0 R"] });
    assert_eq!(walk(listed, &[(6, leaf())]), twice, "listed twice");
    let cycle = [(5, json!({ "/Kids": ["5 0 R"] }))];
    assert_eq!(walk(json!({ "/Fields": ["5 0 R"] }), &cycle), twice);
    // Direct fields sharing one indirect /Kids array.
    let shared = json!({ "/Kids": "8 0 R" });
    let arr = [(8, json!([shared.clone(), shared]))];
    assert_eq!(walk(json!({ "/Fields": "8 0 R" }), &arr), twice);
    // R118: pdfium reads the same rewrite, so null and dangling entries
    // (what qpdf could not resolve) lead nowhere for it either.
    let kid = [(5, json!({ "/Kids": [null, null] }))];
    assert_eq!(walk(json!({ "/Fields": ["5 0 R"] }), &kid), "ok");
    assert_eq!(walk(json!({ "/Fields": ["9 0 R"] }), &[]), "ok");
    let kids = [(5, json!({ "/Kids": null }))];
    assert_eq!(walk(json!({ "/Fields": ["5 0 R"] }), &kids), "ok");
    assert_eq!(walk(json!({ "/Fields": null }), &[]), "ok");
}

#[test]
fn a_long_chain_is_walked_without_recursion() {
    let n = 100_000;
    let objs: Vec<(u32, Value)> = (0..n)
        .map(|i| (10 + i, json!({ "/Kids": [format!("{} 0 R", 11 + i)] })))
        .chain([(10 + n, leaf())])
        .collect();
    assert_eq!(walk(json!({ "/Fields": ["10 0 R"] }), &objs), "ok");
}

#[test]
fn page_widgets_are_walked_into_the_same_tree() {
    let twice = "damaged file: form field tree reaches a field twice";
    let none = json!({ "/Fields": [] });
    let dag = [(5, widget(json!(["6 0 R", "6 0 R"]))), (6, leaf())];
    assert_eq!(walk_annots(none.clone(), json!(["5 0 R"]), &dag), twice);
    // A direct widget, and one not a widget (pdfium loads no field).
    let direct = json!([widget(json!(["6 0 R", "6 0 R"]))]);
    assert_eq!(walk_annots(none.clone(), direct, &[(6, leaf())]), twice);
    let note = [(
        5,
        json!({ "/Subtype": "/Text", "/Kids": ["6 0 R", "6 0 R"] }),
    )];
    assert_eq!(walk_annots(none.clone(), json!(["5 0 R"]), &note), "ok");
    // A field (here itself a widget) and its widgets, listed in /Fields
    // and on the page.
    let tree = [
        (5, widget(json!(["6 0 R", "7 0 R"]))),
        (6, widget(json!([]))),
        (7, widget(json!(["8 0 R"]))),
        (8, leaf()),
    ];
    let fields = json!({ "/Fields": ["5 0 R"] });
    let both = json!(["6 0 R", "7 0 R"]);
    assert_eq!(walk_annots(fields.clone(), both, &tree), "ok");
    // R117: listed twice, pdfium walks it twice.
    let again = json!(["6 0 R", "7 0 R", "6 0 R"]);
    assert_eq!(walk_annots(fields, again, &tree), twice);
    assert_eq!(
        walk_annots(none.clone(), json!(["7 0 R", "7 0 R"]), &tree),
        twice
    );
    // A page widget later reached as a kid once is the same node; twice is not.
    let up = json!(["7 0 R", "5 0 R"]);
    assert_eq!(walk_annots(none.clone(), up, &tree), "ok");
    let mut two = tree.to_vec();
    two[0].1 = widget(json!(["7 0 R", "7 0 R"]));
    assert_eq!(
        walk_annots(none.clone(), json!(["7 0 R", "5 0 R"]), &two),
        twice
    );
    let lost = [(5, widget(json!([null])))];
    assert_eq!(walk_annots(none.clone(), json!(["5 0 R"]), &lost), "ok");
}

#[test]
fn null_or_dangling_page_annots_are_skipped() {
    let none = json!({ "/Fields": [] });
    let w = [(5, widget(json!([]))), (6, json!("/Widget"))];
    // R118: as pdfium reads the rewrite, an element or list qpdf read as
    // null holds no widget; the widgets around it are still walked.
    assert_eq!(walk_annots(none.clone(), json!([null]), &w), "ok");
    assert_eq!(walk_annots(none.clone(), json!(["9 0 R"]), &w), "ok");
    assert_eq!(walk_annots(none.clone(), json!("9 0 R"), &w), "ok");
    assert_eq!(
        walk_annots(none.clone(), json!("5 0 R"), &w),
        "ok",
        "no array"
    );
    let twice = "damaged file: form field tree reaches a field twice";
    let both = json!([null, "5 0 R", null, "5 0 R"]);
    assert_eq!(walk_annots(none, both, &w), twice);
}

/// Fields 10.. as a /Parent chain of `n` (10 the foot, no /Kids).
fn chain(n: u32) -> Vec<(u32, Value)> {
    (0..n)
        .map(|i| match i + 1 < n {
            true => (
                10 + i,
                json!({ "/T": "(p)", "/Parent": format!("{} 0 R", 11 + i) }),
            ),
            false => (10 + i, json!({ "/T": "(p)" })),
        })
        .collect()
}

#[test]
fn a_parent_chain_longer_than_pdfium_nests_fields_fails() {
    let deep = "damaged file: form field nesting is deeper than 32";
    let none = json!({ "/Fields": [] });
    let w = || json!({ "/Subtype": "/Widget", "/T": "(w)", "/Parent": "10 0 R" });
    for (n, want) in [(32, "ok"), (33, deep)] {
        let mut objs = chain(n);
        objs.push((5, w()));
        // A page widget (listed or direct) and a field reached from /Fields.
        assert_eq!(walk_annots(none.clone(), json!(["5 0 R"]), &objs), want);
        assert_eq!(walk_annots(none.clone(), json!([w()]), &objs), want);
        assert_eq!(walk(json!({ "/Fields": ["5 0 R"] }), &objs), want);
        let kid = [(4, json!({ "/Kids": ["5 0 R"] }))];
        let objs = [&objs[..], &kid].concat();
        assert_eq!(walk(json!({ "/Fields": ["4 0 R"] }), &objs), want, "{n}");
    }
    let cycle = [
        (5, json!({ "/Parent": "6 0 R" })),
        (6, json!({ "/Parent": "5 0 R" })),
    ];
    assert_eq!(walk(json!({ "/Fields": ["5 0 R"] }), &cycle), deep);
}
