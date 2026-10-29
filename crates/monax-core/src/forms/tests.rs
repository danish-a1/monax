//! Field tree, plan and output check on hand-built qpdf JSON documents.
use super::fill::{drawn, unknown};
use super::set::Plan;
use super::tree::nodes;
use super::{Field, FieldKind as K};
use crate::content::qjson::Doc;
use crate::pages::Rect;
use serde_json::{Map, Value, json};
use std::collections::HashSet;

/// Catalog 1 with `/AcroForm` `form`, one page 3 listing `annots`, and
/// `objs` (`N` -> value of `N 0 R`).
fn doc(form: Value, annots: &[&str], objs: &[(u32, Value)]) -> Doc {
    let mut o = Map::new();
    o.insert("trailer".into(), json!({ "value": { "/Root": "1 0 R" } }));
    let cat = json!({ "/AcroForm": form, "/Pages": "2 0 R", "/NeedsRendering": true });
    o.insert("obj:1 0 R".into(), json!({ "value": cat }));
    o.insert(
        "obj:2 0 R".into(),
        json!({ "value": { "/Kids": ["3 0 R"] } }),
    );
    o.insert(
        "obj:3 0 R".into(),
        json!({ "value": { "/Annots": annots } }),
    );
    for (n, v) in objs {
        o.insert(format!("obj:{n} 0 R"), json!({ "value": v }));
    }
    Doc::of(Value::Object(o))
}

fn names(d: &Doc) -> Vec<String> {
    nodes(d).into_iter().map(|n| n.field.name).collect()
}

#[test]
fn the_walk_inherits_and_skips_what_is_no_field() {
    let d = doc(
        json!({ "/Fields": ["10 0 R", "10 0 R", { "/T": "u:direct", "/FT": "/Tx" }, "13 0 R"] }),
        &["11 0 R", "12 0 R"],
        &[
            (
                10,
                json!({ "/T": "u:p", "/FT": "/Ch", "/Ff": 2097152, "/V": ["u:b", "u:a"],
                "/Kids": ["11 0 R", "14 0 R"] }),
            ),
            (
                11,
                json!({ "/T": "u:kid", "/Ff": 1, "/Opt": ["u:a", ["u:b", "u:Bee"]],
                "/Rect": [9, 8, 1, 2] }),
            ),
            (
                12,
                json!({ "/T": "b:feff004e", "/FT": "/Tx", "/MaxLen": 3, "/Parent": "13 0 R" }),
            ),
            (
                13,
                json!({ "/T": "u:t", "/FT": "/Tx", "/Kids": ["12 0 R"] }),
            ),
            (14, json!({ "/T": "u:nokind", "/FT": "/Zz" })),
        ],
    );
    let all = nodes(&d);
    let list = Field {
        name: "p.kid".into(),
        kind: K::List,
        page: Some(0),
        rect: Some(Rect {
            left: 1.0,
            bottom: 2.0,
            right: 9.0,
            top: 8.0,
        }),
        value: "b".into(),
        options: vec!["a".into(), "b".into()],
        read_only: true,
    };
    assert_eq!(all[0].field, list);
    assert_eq!(names(&d), ["p.kid", "t.N"]);
    assert_eq!(all[1].max_len, Some(3));
}

#[test]
fn radio_export_values_come_from_opt_and_the_value_maps_back() {
    let w =
        |s: &str| json!({ "/Parent": "10 0 R", "/AP": { "/N": { s: "9 0 R", "/Off": "9 0 R" } } });
    let d = doc(
        json!({ "/Fields": ["10 0 R"] }),
        &["11 0 R"],
        &[
            (
                10,
                json!({ "/T": "u:r", "/FT": "/Btn", "/Ff": 32768, "/V": "/1",
                "/Opt": ["u:Ja", "u:Nein"], "/Kids": ["11 0 R", "12 0 R"] }),
            ),
            (11, w("/0")),
            (12, w("/1")),
        ],
    );
    let n = &nodes(&d)[0];
    assert_eq!(
        (n.field.value.as_str(), &n.field.options[..]),
        ("Nein", &["Ja".to_string(), "Nein".into()][..])
    );
    assert_eq!(n.widgets[1].page, None, "12 is on no page");
    let mut plan = Plan::new(&d);
    plan.set(n, "Ja").unwrap();
    let up = plan.updates();
    assert_eq!(up["obj:10 0 R"]["value"]["/V"], "/0");
    assert_eq!(up["obj:11 0 R"]["value"]["/AS"], "/0");
    assert_eq!(up["obj:12 0 R"]["value"]["/AS"], "/Off");
}

#[test]
fn a_field_with_no_placed_widget_or_no_on_state_is_refused() {
    let d = doc(
        json!({ "/Fields": ["10 0 R", "11 0 R"] }),
        &["11 0 R"],
        &[
            (10, json!({ "/T": "u:hidden", "/FT": "/Tx" })),
            (11, json!({ "/T": "u:box", "/FT": "/Btn" })),
        ],
    );
    let all = nodes(&d);
    let mut plan = Plan::new(&d);
    let e = |r: Result<(), crate::pages::PagesError>| r.unwrap_err().to_string();
    assert_eq!(
        e(plan.set(&all[0], "x")),
        "invalid input: field \"hidden\" has no widget on a page, so its value would not show"
    );
    assert_eq!(
        e(plan.set(&all[1], "true")),
        "invalid input: the value for field \"box\" is not one of its values (Off)"
    );
}

#[test]
fn xfa_goes_from_an_indirect_or_direct_form() {
    let indirect = doc(
        json!("20 0 R"),
        &[],
        &[(20, json!({ "/Fields": [], "/XFA": "u:x" }))],
    );
    let mut plan = Plan::new(&indirect);
    assert!(plan.drop_xfa());
    let up = plan.updates();
    assert_eq!(up["obj:20 0 R"]["value"], json!({ "/Fields": [] }));
    assert!(up["obj:1 0 R"]["value"].get("/NeedsRendering").is_none());
    let direct = doc(json!({ "/XFA": "u:x" }), &[], &[]);
    let mut plan = Plan::new(&direct);
    assert!(plan.drop_xfa());
    assert_eq!(plan.updates()["obj:1 0 R"]["value"]["/AcroForm"], json!({}));
    assert!(!Plan::new(&doc(json!({}), &[], &[])).drop_xfa());
}

#[test]
fn unknown_names_list_at_most_50_valid_ones() {
    let refs: Vec<String> = (100..160).map(|n| format!("{n} 0 R")).collect();
    let objs: Vec<(u32, Value)> = (100..160)
        .map(|n| (n, json!({ "/T": format!("u:f{n}"), "/FT": "/Tx" })))
        .collect();
    let d = doc(json!({ "/Fields": refs }), &[], &objs);
    let msg = unknown("x", &nodes(&d)).to_string();
    assert!(
        msg.starts_with("invalid input: unknown field \"x\"; valid fields: f100, f101,"),
        "{msg}"
    );
    assert!(msg.ends_with("f148, f149, ... and 10 more"), "{msg}");
}

#[test]
fn the_output_check_needs_an_appearance_for_each_filled_widget() {
    let d = doc(
        json!({ "/Fields": ["10 0 R", "11 0 R", "12 0 R"] }),
        &["10 0 R", "11 0 R", "12 0 R"],
        &[
            (
                10,
                json!({ "/T": "u:t", "/FT": "/Tx", "/AP": { "/N": "9 0 R" } }),
            ),
            (11, json!({ "/T": "u:bare", "/FT": "/Tx" })),
            (
                12,
                json!({ "/T": "u:c", "/FT": "/Btn", "/AS": "/Yes", "/AP": { "/N": { "/Off": "9 0 R" } } }),
            ),
            (9, json!({})),
        ],
    );
    let check = |n: &str| drawn(&d, &HashSet::from([n])).map_err(|e| e.to_string());
    assert_eq!(check("t"), Ok(()));
    let not = |n: &str| {
        Err(format!(
            "form field \"{n}\" could not be drawn with its new value; nothing was written"
        ))
    };
    assert_eq!(check("bare"), not("bare"));
    assert_eq!(check("c"), not("c"));
}

mod stream;
