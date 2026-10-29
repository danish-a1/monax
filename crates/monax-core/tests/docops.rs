//! set_metadata, flatten, delete_annotations, repair, linearize.
mod common;
use common::docs::{annots, assert_clean, info, qpdf};
use common::{copy_fixture, fixture, names, setup, words};
use monax_core::content::{self, CompressOpts, Metadata};
use monax_core::pages::{self, PagesError};
use monax_core::pdfium;
use std::path::{Path, PathBuf};

fn text(p: &Path, page: usize) -> String {
    pdfium::extract_text(p, page..=page).unwrap()
}

#[test]
fn metadata_round_trips_none_keeps_and_empty_clears() {
    require_qpdf!();
    let (dir, three) = setup();
    let m = Metadata {
        title: Some("Report (draft) é".into()),
        author: Some("Ann".into()),
        subject: Some("S".into()),
        keywords: Some("a, b".into()),
        creator: Some("C".into()),
        producer: Some("P".into()),
    };
    let one = content::set_metadata(&three, &dir.path().join("1.pdf"), m).unwrap();
    let want = [
        ("/Author", "Ann"),
        ("/Creator", "C"),
        ("/Keywords", "a, b"),
        ("/Producer", "P"),
        ("/Subject", "S"),
        ("/Title", "Report (draft) é"),
    ];
    let want: Vec<(String, String)> = want
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    assert_eq!(info(&one), want);
    let edit = Metadata {
        author: Some(String::new()),
        title: Some("New".into()),
        ..Metadata::default()
    };
    let two = content::set_metadata(&one, &dir.path().join("2.pdf"), edit).unwrap();
    let got = info(&two);
    assert!(!got.iter().any(|(k, _)| k == "/Author"), "{got:?}");
    assert!(got.contains(&("/Title".into(), "New".into())));
    assert!(got.contains(&("/Producer".into(), "P".into())));
    assert_eq!(words(&two), ["one", "two", "three"]);
}

#[test]
fn flatten_burns_fields_and_annotations_into_the_content() {
    require_qpdf!();
    let (dir, a) = copy_fixture("annots.pdf", "a.pdf");
    assert!(!text(&a, 0).contains("Hello"));
    let out = content::flatten(&a, &dir.path().join("f.pdf")).unwrap();
    assert_eq!(annots(&out), [Vec::<String>::new(), vec![]]);
    assert!(text(&out, 0).contains("Hello"), "{}", text(&out, 0));
    assert!(text(&out, 1).contains("World"));
    assert_eq!(
        common::diff_box(&a, &out, 0),
        None,
        "flattened page looks different"
    );
}

#[test]
fn delete_annotations_on_all_or_some_pages() {
    require_qpdf!();
    let (dir, a) = copy_fixture("annots.pdf", "a.pdf");
    assert_eq!(annots(&a)[0], ["/Widget", "/Square", "/Link", "/Text"]);
    // Form fields (widgets) are not annotations to delete (R82).
    let widget = || vec!["/Widget".to_string()];
    let all = content::delete_annotations(&a, &dir.path().join("all.pdf"), None).unwrap();
    assert_eq!(annots(&all), [widget(), widget()]);
    let first = content::delete_annotations(&a, &dir.path().join("p.pdf"), Some(&[0])).unwrap();
    assert_eq!(annots(&first), [widget(), widget()]);
    // The page-0 field still renders its value; the square is gone.
    let field = common::diff_box_in(&a, &first, 0, 20..141, 80..101);
    assert_eq!(field, None, "widget appearance changed");
    assert!(common::diff_box_in(&a, &first, 0, 150..251, 150..251).is_some());
    // The untouched page is unchanged.
    assert!(common::diff_box(&a, &first, 1).is_none());
    let oor = content::delete_annotations(&a, &dir.path().join("x.pdf"), Some(&[2]));
    assert!(
        matches!(oor, Err(PagesError::PageOutOfRange { page: 2, count: 2 })),
        "{oor:?}"
    );
    assert!(!names(dir.path()).contains(&"x.pdf".to_string()));
}

/// `three-pages.pdf` cut off inside its xref table (an interrupted
/// download): pdfium refuses it, qpdf can rebuild it.
fn truncated(dir: &Path) -> PathBuf {
    let bytes = std::fs::read(fixture("three-pages.pdf")).unwrap();
    let xref = bytes.windows(5).position(|w| w == b"\nxref").unwrap();
    let p = dir.join("trunc.pdf");
    std::fs::write(&p, &bytes[..xref + 20]).unwrap();
    p
}

#[test]
fn repair_succeeds_where_every_other_op_reports_damaged() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let bad = truncated(dir.path());
    let o = dir.path().join("o.pdf");
    let damaged = |r: Result<PathBuf, PagesError>| matches!(r, Err(PagesError::Damaged));
    assert!(damaged(pages::extract(&bad, &[0], &o)));
    assert!(damaged(content::compress(
        &bad,
        &o,
        CompressOpts::default()
    )));
    assert!(damaged(content::set_metadata(
        &bad,
        &o,
        Metadata::default()
    )));
    assert!(damaged(content::flatten(&bad, &o)));
    assert!(damaged(content::delete_annotations(&bad, &o, None)));
    assert!(damaged(content::linearize(&bad, &o)));
    let fixed = content::repair(&bad, &dir.path().join("fixed.pdf")).unwrap();
    assert_eq!(words(&fixed), ["one", "two", "three"]);
    assert_clean(&fixed);
    // Nothing recoverable: qpdf's own reason, no file.
    std::fs::write(&o, b"%PDF-1.4\n").unwrap();
    let r = content::repair(&o, &dir.path().join("none.pdf"));
    assert!(
        matches!(&r, Err(PagesError::Qpdf(m)) if m.contains("trailer")),
        "{r:?}"
    );
    assert_eq!(names(dir.path()), ["fixed.pdf", "o.pdf", "trunc.pdf"]);
}

#[test]
fn linearize_passes_qpdf_check_linearization() {
    require_qpdf!();
    let (dir, three) = setup();
    let out = content::linearize(&three, &dir.path().join("l.pdf")).unwrap();
    let report = qpdf(&["--check-linearization", out.to_str().unwrap()]);
    assert!(report.contains("no linearization errors"), "{report}");
    assert_eq!(words(&out), ["one", "two", "three"]);
}

#[test]
fn every_op_keeps_rotation_and_crop_boxes() {
    require_qpdf!();
    let (dir, rc) = copy_fixture("rotated-crop.pdf", "rc.pdf");
    let geometry = |p: &Path| -> Vec<(u16, (f32, f32, f32, f32))> {
        let n = pdfium::page_count(p).unwrap();
        (0..n)
            .map(|i| {
                (
                    pdfium::rotation(p, i).unwrap(),
                    pdfium::crop_box(p, i).unwrap(),
                )
            })
            .collect()
    };
    let o = |n: &str| dir.path().join(n);
    let outs = [
        content::compress(&rc, &o("c.pdf"), CompressOpts::default()),
        content::set_metadata(&rc, &o("m.pdf"), Metadata::default()),
        content::flatten(&rc, &o("f.pdf")),
        content::delete_annotations(&rc, &o("d.pdf"), None),
        content::repair(&rc, &o("r.pdf")),
        content::linearize(&rc, &o("l.pdf")),
    ];
    for out in outs {
        let out = out.unwrap();
        assert_eq!(geometry(&out), geometry(&rc), "{}", out.display());
        assert!(
            common::diff_box(&rc, &out, 0).is_none(),
            "{}",
            out.display()
        );
        assert_clean(&out);
    }
}
