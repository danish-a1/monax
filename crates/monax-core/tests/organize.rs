mod common;
use common::{copy_fixture, fixture, rotations, setup, words};
use monax_core::pages::{self, PagePlan, PagesError};
use monax_core::pdfium;

fn plan(v: &[(usize, i16)]) -> Vec<PagePlan> {
    v.iter()
        .map(|&(page, rotate)| PagePlan { page, rotate })
        .collect()
}

#[test]
fn order_with_deletion_and_per_page_rotation() {
    require_qpdf!();
    let (dir, src) = setup();
    let out = dir.path().join("o.pdf");
    let got = pages::organize(&src, &plan(&[(2, 90), (0, 0)]), &out).unwrap();
    assert_eq!(words(&got), ["three", "one"]);
    assert_eq!(rotations(&got), [90, 0]);
}

#[test]
fn negative_rotation_is_counter_clockwise() {
    require_qpdf!();
    let (dir, src) = setup();
    let got = pages::organize(
        &src,
        &plan(&[(0, -90), (1, 180)]),
        &dir.path().join("o.pdf"),
    )
    .unwrap();
    assert_eq!(rotations(&got), [270, 180]);
}

#[test]
fn rotation_accumulates_on_rotated_page() {
    require_qpdf!();
    let (dir, src) = copy_fixture("rotated-crop.pdf", "rc.pdf");
    let before = pdfium::rotation(&src, 0).unwrap();
    assert_eq!(before, 90);
    let got = pages::organize(&src, &plan(&[(0, 90)]), &dir.path().join("o.pdf")).unwrap();
    assert_eq!(rotations(&got)[0], 180);
}

#[test]
fn bad_plans_are_rejected() {
    require_qpdf!();
    let (dir, src) = setup();
    let out = dir.path().join("o.pdf");
    let r = pages::organize(&src, &[], &out);
    assert!(matches!(r, Err(PagesError::InvalidInput(_))));
    let r = pages::organize(&src, &plan(&[(0, 45)]), &out);
    assert!(matches!(r, Err(PagesError::InvalidInput(_))));
    let r = pages::organize(&src, &plan(&[(3, 0)]), &out);
    assert!(matches!(
        r,
        Err(PagesError::PageOutOfRange { page: 3, count: 3 })
    ));
    assert!(!out.exists());
}

#[test]
fn guards_overwrite_collision_encrypted_damaged() {
    require_qpdf!();
    let (dir, src) = setup();
    let p = plan(&[(0, 0)]);
    let r = pages::organize(&src, &p, &src);
    assert!(matches!(r, Err(PagesError::InvalidInput(_))));
    let out = dir.path().join("o.pdf");
    std::fs::write(&out, b"keep").unwrap();
    let got = pages::organize(&src, &p, &out).unwrap();
    assert_eq!(got, dir.path().join("o-1.pdf"));
    assert_eq!(std::fs::read(&out).unwrap(), b"keep");

    let enc = fixture("encrypted.pdf");
    let r = pages::organize(&enc, &p, &dir.path().join("e.pdf"));
    assert!(matches!(r, Err(PagesError::EncryptedInput)));
    let bad = dir.path().join("bad.pdf");
    std::fs::write(&bad, b"not a pdf").unwrap();
    let r = pages::organize(&bad, &p, &dir.path().join("b.pdf"));
    assert!(matches!(r, Err(PagesError::Damaged)));
    assert!(!dir.path().join("e.pdf").exists() && !dir.path().join("b.pdf").exists());
}
