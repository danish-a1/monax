mod common;
use common::{fixture, rotations, setup, words};
use monax_core::pages::{self, MergeOpts, PagesError};
use monax_core::pdfium;
use std::path::Path;

#[test]
fn merge_keeps_input_order() {
    require_qpdf!();
    let (dir, three) = setup();
    let last = pages::extract(&three, &[2], &dir.path().join("last.pdf")).unwrap();
    let out = dir.path().join("merged.pdf");
    let got = pages::merge(&[last, three], &out, MergeOpts::default()).unwrap();
    assert_eq!(got, out);
    assert_eq!(words(&got), ["three", "one", "two", "three"]);
}

#[test]
fn merge_blank_before_even() {
    require_qpdf!();
    let (dir, three) = setup();
    let opts = MergeOpts {
        blank_before_even: true,
    };
    let out = dir.path().join("m.pdf");
    let got = pages::merge(&[three.clone(), three.clone()], &out, opts).unwrap();
    // A blank after the odd first input; none after the last input.
    assert_eq!(
        words(&got),
        ["one", "two", "three", "", "one", "two", "three"]
    );
    let blank = pdfium::page_size(&got, 3).unwrap();
    assert_eq!(blank, pdfium::page_size(&three, 2).unwrap());
}

#[test]
fn merge_blank_matches_visible_size_of_rotated_page() {
    require_qpdf!();
    let (dir, three) = setup();
    let rot = fixture("rotated-crop.pdf");
    let opts = MergeOpts {
        blank_before_even: true,
    };
    let got = pages::merge(&[rot.clone(), three], &dir.path().join("m.pdf"), opts).unwrap();
    assert_eq!(pdfium::page_count(&got).unwrap(), 5);
    assert_eq!(pdfium::page_size(&got, 1).unwrap(), (100., 200.));
    assert_eq!(pdfium::rotation(&got, 1).unwrap(), 0);
}

#[test]
fn extract_keeps_document_order() {
    require_qpdf!();
    let (dir, three) = setup();
    let got = pages::extract(&three, &[2, 0, 2], &dir.path().join("x.pdf")).unwrap();
    assert_eq!(words(&got), ["one", "three"]);
}

#[test]
fn delete_pages_and_refuse_all() {
    require_qpdf!();
    let (dir, three) = setup();
    let got = pages::delete(&three, &[1], &dir.path().join("d.pdf")).unwrap();
    assert_eq!(words(&got), ["one", "three"]);
    let all = pages::delete(&three, &[0, 1, 2], &dir.path().join("e.pdf"));
    assert!(matches!(all, Err(PagesError::InvalidInput(_))), "{all:?}");
    assert!(!dir.path().join("e.pdf").exists());
}

#[test]
fn rotate_accumulates_with_existing_rotate() {
    require_qpdf!();
    let (dir, three) = setup();
    let rot = fixture("rotated-crop.pdf");
    assert_eq!(pdfium::rotation(&rot, 0).unwrap(), 90);
    let got = pages::rotate(&rot, &[0], 90, &dir.path().join("r.pdf")).unwrap();
    assert_eq!(rotations(&got), [180]);
    let got = pages::rotate(&three, &[1], -90, &dir.path().join("s.pdf")).unwrap();
    assert_eq!(rotations(&got), [0, 270, 0]);
    assert_eq!(words(&got), ["one", "two", "three"]);
}

#[test]
fn rotate_rejects_non_right_angles() {
    require_qpdf!();
    let (dir, three) = setup();
    for deg in [45, 0, 100] {
        let r = pages::rotate(&three, &[0], deg, &dir.path().join("r.pdf"));
        assert!(
            matches!(r, Err(PagesError::InvalidInput(_))),
            "{deg}: {r:?}"
        );
    }
}

#[test]
fn reorder_allows_duplicates() {
    require_qpdf!();
    let (dir, three) = setup();
    let got = pages::reorder(&three, &[2, 0, 0], &dir.path().join("o.pdf")).unwrap();
    assert_eq!(words(&got), ["three", "one", "one"]);
}

#[test]
fn alternate_mix_interleaves_and_appends_leftovers() {
    require_qpdf!();
    let (dir, a) = setup();
    // b = pages one, two of `a`, rotated 90 so b's pages are distinguishable.
    let two = pages::extract(&a, &[0, 1], &dir.path().join("two.pdf")).unwrap();
    let b = pages::rotate(&two, &[0, 1], 90, &dir.path().join("b.pdf")).unwrap();
    let pairs =
        |p: &Path| -> Vec<(String, u16)> { words(p).into_iter().zip(rotations(p)).collect() };
    let s = |w: &str, r: u16| (w.to_string(), r);

    let got = pages::alternate_mix(&a, &b, false, &dir.path().join("m.pdf")).unwrap();
    let want = [
        s("one", 0),
        s("one", 90),
        s("two", 0),
        s("two", 90),
        s("three", 0),
    ];
    assert_eq!(pairs(&got), want);

    let got = pages::alternate_mix(&a, &b, true, &dir.path().join("n.pdf")).unwrap();
    let want = [
        s("one", 0),
        s("two", 90),
        s("two", 0),
        s("one", 90),
        s("three", 0),
    ];
    assert_eq!(pairs(&got), want);
}
