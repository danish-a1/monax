//! R119: the redaction walk ([`original`]): pdfium opens the original.
use super::super::*;
use super::{Value, doc, widget};
use serde_json::json;

/// The message of [`original`]'s error, or "ok".
fn strict(form: Value, annots: Value, objs: &[(u32, Value)]) -> String {
    let p = Path::new("/x.pdf");
    original(p, &doc(form, annots, objs)).map_or_else(|e| e.to_string(), |_| "ok".into())
}

#[test]
fn for_redaction_a_reference_qpdf_cannot_resolve_fails_closed() {
    let unknown = "form field check failed: a field reference does not resolve";
    let (none, w) = (json!({ "/Fields": [] }), [(5, widget(json!([])))]);
    let kid = [(5, json!({ "/Kids": [null] }))];
    for (form, annots, objs, want) in [
        (
            json!({ "/Fields": ["5 0 R"] }),
            Value::Null,
            &kid[..],
            unknown,
        ),
        (json!({ "/Fields": ["9 0 R"] }), Value::Null, &[], unknown),
        (json!({ "/Fields": null }), Value::Null, &[], unknown),
        (Value::Null, Value::Null, &[], unknown),
        (none.clone(), json!([null]), &w, unknown),
        (none.clone(), json!("9 0 R"), &w, unknown),
        (
            none.clone(),
            json!(["5 0 R"]),
            &[(5, widget(Value::Null))],
            unknown,
        ),
        (none.clone(), json!(["5 0 R"]), &w, "ok"),
        (
            none,
            json!(["5 0 R", "5 0 R"]),
            &w,
            "damaged file: form field tree reaches a field twice",
        ),
    ] {
        assert_eq!(
            strict(form.clone(), annots.clone(), objs),
            want,
            "{form} {annots}"
        );
    }
}
