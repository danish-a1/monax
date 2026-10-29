//! AcroForm list and fill: every kind listed with its rect and options;
//! filled values read back, show in a render and live in the file's own
//! appearance streams.
mod common;
use common::copy_fixture;
use common::forms::{field, form, fv, ink_in, widget};
use monax_core::content::flatten;
use monax_core::forms::{Field, FieldKind as K, fill, list_fields};
use monax_core::pages::Rect;
use monax_core::pdfium::extract_text;

fn f(name: &str, kind: K, [l, b, r, t]: [f32; 4], value: &str, options: &[&str]) -> Field {
    Field {
        name: name.into(),
        kind,
        page: Some(0),
        rect: Some(Rect {
            left: l,
            bottom: b,
            right: r,
            top: t,
        }),
        value: value.into(),
        options: options.iter().map(|s| s.to_string()).collect(),
        read_only: false,
    }
}

#[test]
fn lists_every_kind_with_its_rect_value_and_options() {
    require_qpdf!();
    let (_d, p) = form();
    let locked = Field {
        read_only: true,
        ..f("locked", K::Text, [150.0, 80.0, 280.0, 100.0], "fixed", &[])
    };
    let want = vec![
        f(
            "person.name",
            K::Text,
            [20.0, 250.0, 180.0, 270.0],
            "Ann",
            &[],
        ),
        f("email", K::Text, [20.0, 220.0, 180.0, 240.0], "", &[]),
        f(
            "agree",
            K::Checkbox,
            [20.0, 190.0, 40.0, 210.0],
            "Off",
            &["Agreed"],
        ),
        f(
            "color",
            K::Radio,
            [20.0, 160.0, 40.0, 180.0],
            "Off",
            &["red", "blue"],
        ),
        f(
            "size",
            K::Combo,
            [20.0, 130.0, 120.0, 150.0],
            "M",
            &["S", "M", "L"],
        ),
        f(
            "fruit",
            K::List,
            [20.0, 80.0, 120.0, 120.0],
            "",
            &["Apple", "Pear"],
        ),
        f("go", K::Button, [150.0, 190.0, 200.0, 210.0], "", &[]),
        f("sig", K::Signature, [150.0, 130.0, 280.0, 160.0], "", &[]),
        locked,
        f(
            "note",
            K::Combo,
            [150.0, 50.0, 280.0, 70.0],
            "",
            &["a", "b"],
        ),
    ];
    assert_eq!(list_fields(&p).unwrap(), want);
}

#[test]
fn lists_two_text_fields_on_their_pages() {
    require_qpdf!();
    let (_d, p) = copy_fixture("annots.pdf", "a.pdf");
    let got: Vec<_> = list_fields(&p)
        .unwrap()
        .into_iter()
        .map(|f| (f.name, f.page, f.value))
        .collect();
    let want = [("name", 0, "Hello"), ("other", 1, "World")]
        .map(|(n, p, v)| (n.to_string(), Some(p), v.to_string()));
    assert_eq!(got, want);
    assert!(
        list_fields(&common::fixture("three-pages.pdf"))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn filled_values_read_back_show_and_live_in_the_file() {
    require_qpdf!();
    let (d, p) = form();
    let before = std::fs::read(&p).unwrap();
    let out = d.path().join("o.pdf");
    let values = [
        fv("person.name", "Zed Ünï"),
        fv("email", "a@b.c"),
        fv("agree", "true"),
        fv("color", "blue"),
        fv("size", "S"),
        fv("fruit", "Pear"),
        fv("note", "free text"),
    ];
    assert_eq!(fill(&p, &out, &values).unwrap(), out);
    assert_eq!(std::fs::read(&p).unwrap(), before, "input untouched");
    for v in &values {
        let want = if v.value == "true" {
            "Agreed"
        } else {
            &v.value
        };
        assert_eq!(field(&out, &v.name).value, want, "{}", v.name);
    }
    assert_eq!(
        field(&out, "locked").value,
        "fixed",
        "unfilled keeps its value"
    );
    let rects = [
        [20, 250, 180, 270],
        [20, 220, 180, 240],
        [20, 190, 40, 210],
        [50, 160, 70, 180],
        [20, 130, 120, 150],
        [20, 80, 120, 120],
        [150, 50, 280, 70],
    ];
    for r in rects {
        assert!(ink_in(&p, &out, r), "no visible change in {r:?}");
    }
    assert!(!ink_in(&p, &out, [20, 160, 40, 180]), "red stays off");
    // The file's own appearance streams carry the values.
    assert!(widget(&out, "email")["/AP"].is_object());
    assert_eq!(widget(&out, "agree")["/AS"], "/Agreed");
}

#[test]
fn a_stale_appearance_is_replaced_by_the_new_value() {
    require_qpdf!();
    let (d, p) = copy_fixture("annots.pdf", "a.pdf");
    let out = fill(&p, &d.path().join("o.pdf"), &[fv("name", "Bye")]).unwrap();
    // Flatten draws each widget's appearance stream into the page.
    let flat = flatten(&out, &d.path().join("f.pdf")).unwrap();
    let text = extract_text(&flat, 0..=0).unwrap();
    assert!(text.contains("Bye") && !text.contains("Hello"), "{text:?}");
    assert_eq!(field(&out, "other").value, "World");
}
