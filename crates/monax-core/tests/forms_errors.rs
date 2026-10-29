//! fill refuses what it cannot set visibly, naming the field and the
//! reason, and writes nothing.
mod common;
use common::forms::{form, fv};
use monax_core::forms::{FieldValue, fill};

/// The message of `fill` on `form.pdf` with `values`; nothing was written.
fn refused(values: &[FieldValue]) -> String {
    let (d, p) = form();
    let e = fill(&p, &d.path().join("o.pdf"), values).unwrap_err();
    assert_eq!(common::names(d.path()), ["form.pdf"]);
    e.to_string()
}

#[test]
fn unknown_and_duplicate_names_are_refused() {
    require_qpdf!();
    let valid =
        "valid fields: person.name, email, agree, color, size, fruit, go, sig, locked, note";
    assert_eq!(
        refused(&[fv("name", "x")]),
        format!("invalid input: unknown field \"name\"; {valid}")
    );
    assert_eq!(
        refused(&[fv("email", "a"), fv("email", "b")]),
        "invalid input: field \"email\" is given twice"
    );
    assert_eq!(refused(&[]), "invalid input: no field values given");
    let (d, p) = common::setup();
    let e = fill(&p, &d.path().join("o.pdf"), &[fv("a", "b")]).unwrap_err();
    assert_eq!(
        e.to_string(),
        "invalid input: unknown field \"a\"; the document has no form fields"
    );
}

#[test]
fn values_a_field_does_not_take_are_refused_with_the_reason() {
    require_qpdf!();
    let cases = [
        (fv("locked", "x"), "field \"locked\" is read-only"),
        (
            fv("go", "x"),
            "field \"go\" is a push button, which holds no value",
        ),
        (
            fv("sig", "x"),
            "field \"sig\" is a signature field, which cannot be filled",
        ),
        (
            fv("email", "abcdefghijk"),
            "the value for field \"email\" is longer than 10 characters",
        ),
        (
            fv("size", "XL"),
            "the value for field \"size\" is not one of its options (S, M, L)",
        ),
        (
            fv("fruit", "Small"),
            "the value for field \"fruit\" is not one of its options (Apple, Pear)",
        ),
        (
            fv("agree", "Yes"),
            "the value for field \"agree\" is not one of its values (Agreed, Off)",
        ),
        (
            fv("color", "true"),
            "the value for field \"color\" is not one of its values (red, blue, Off)",
        ),
    ];
    for (v, why) in cases {
        assert_eq!(
            refused(&[fv("person.name", "ok"), v]),
            format!("invalid input: {why}")
        );
    }
}

#[test]
fn off_and_false_clear_a_checkbox_and_maxlen_counts_characters() {
    require_qpdf!();
    let (d, p) = form();
    let on = fill(
        &p,
        &d.path().join("on.pdf"),
        &[fv("agree", "Agreed"), fv("color", "red")],
    )
    .unwrap();
    let ten = "ÄÖÜäöüßéèê"; // 10 characters, 20 bytes
    for (i, off) in ["Off", "false"].into_iter().enumerate() {
        let out = d.path().join(format!("off{i}.pdf"));
        let vals = [fv("agree", off), fv("color", "Off"), fv("email", ten)];
        let out = fill(&on, &out, &vals).unwrap();
        assert_eq!(common::forms::field(&out, "agree").value, "Off");
        assert_eq!(common::forms::field(&out, "color").value, "Off");
        assert_eq!(common::forms::field(&out, "email").value, ten);
    }
}

#[test]
fn a_checkbox_drawn_by_one_stream_offers_no_value_and_writes_nothing() {
    require_qpdf!();
    let (d, p) = form();
    let objs = common::docs::objects(&p);
    let mut w = common::docs::get(&objs, &serde_json::json!("13 0 R")).clone();
    w["/AP"] = serde_json::json!({ "/N": "31 0 R" }); // a stream, no states
    let src = common::forms::derive(
        &p,
        "s.pdf",
        serde_json::json!({ "obj:13 0 R": { "value": w } }),
    );
    assert!(common::forms::field(&src, "agree").options.is_empty());
    let e = fill(&src, &d.path().join("o.pdf"), &[fv("agree", "true")]).unwrap_err();
    let want = "invalid input: the value for field \"agree\" is not one of its values (Off)";
    assert_eq!(e.to_string(), want);
    assert_eq!(common::names(d.path()), ["form.pdf", "s.pdf"]);
}
