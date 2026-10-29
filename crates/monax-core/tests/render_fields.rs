//! R113: every pdfium render (render, thumbnail, flatten, the redaction
//! burn) and every walk of the widgets (delete annotations, compress
//! discarding media) refuses a form whose field tree reaches a field twice,
//! by object number as pdfium resolves it, before pdfium walks it once per
//! path; a healthy tree renders as before.
mod common;
use common::probe14::probe14_pdf;
use common::probe16::{gen1_fields, rebuilt_fields};
use common::secret::rd;
use monax_core::Secret;
use monax_core::content::{CompressOpts, compress, delete_annotations, flatten, repair};
use monax_core::pdfium::{render, thumbnail_png, thumbnail_png_with_password};
use monax_core::security::redact;
use std::time::{Duration, Instant};

const REVISIT: &str = "damaged file: form field tree reaches a field twice";

#[test]
fn a_field_dag_is_refused_by_every_render_in_time() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    // 26 levels: pdfium alone takes about 11 s per render.
    let src = probe14_pdf(dir.path(), "fielddag:26");
    let out = dir.path().join("out.pdf");
    let t = Instant::now();
    let discard = CompressOpts {
        discard_media: true,
        ..CompressOpts::default()
    };
    let g1 = gen1_fields(&src);
    let errs = [
        render(&src, 0, 72).map(drop).map_err(|e| e.to_string()),
        delete_annotations(&src, &out, None)
            .map(drop)
            .map_err(|e| e.to_string()),
        compress(&src, &out, discard)
            .map(drop)
            .map_err(|e| e.to_string()),
        thumbnail_png(&src, 1, 64)
            .map(drop)
            .map_err(|e| e.to_string()),
        flatten(&src, &out).map(drop).map_err(|e| e.to_string()),
        redact(&src, &out, &[rd(1, 10, 10, 50, 50)])
            .map(drop)
            .map_err(|e| e.to_string()),
    ];
    assert_eq!(errs, [(); 6].map(|_| Err(REVISIT.to_string())));
    // Fields moved to generation 1 under generation-0 /Kids: qpdf writes
    // those references as null, and pdfium draws qpdf's rewrite (R118).
    thumbnail_png(&g1, 1, 64).unwrap();
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
    assert!(!out.exists());
}

#[test]
fn a_widget_dag_listed_only_in_a_page_annots_is_refused_in_time() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    // 22 levels below a widget no /Fields entry reaches: pdfium loads it
    // from the page's /Annots, once per path.
    let src = probe14_pdf(dir.path(), "widgetdag:22");
    let out = dir.path().join("out.pdf");
    let t = Instant::now();
    let errs = [
        render(&src, 1, 72).map(drop).map_err(|e| e.to_string()),
        thumbnail_png(&src, 1, 64)
            .map(drop)
            .map_err(|e| e.to_string()),
        flatten(&src, &out).map(drop).map_err(|e| e.to_string()),
    ];
    assert_eq!(errs, [(); 3].map(|_| Err(REVISIT.to_string())));
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
    assert!(!out.exists());
}

#[test]
fn a_form_read_through_a_rebuilt_xref_is_refused_until_repaired() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    // qpdf reads a harmless field where pdfium reads a 22-level DAG.
    let src = rebuilt_fields(&probe14_pdf(dir.path(), "fielddag:22"));
    let out = dir.path().join("out.pdf");
    let t = Instant::now();
    let errs = [
        render(&src, 0, 72).map(drop).map_err(|e| e.to_string()),
        thumbnail_png(&src, 1, 64)
            .map(drop)
            .map_err(|e| e.to_string()),
        flatten(&src, &out).map(drop).map_err(|e| e.to_string()),
        delete_annotations(&src, &out, None)
            .map(drop)
            .map_err(|e| e.to_string()),
    ];
    let repair_first = "damaged file: repair it first";
    assert_eq!(errs, [(); 4].map(|_| Err(repair_first.to_string())));
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
    assert!(!out.exists());
    // Repair keeps qpdf's reading: the harmless field.
    let fixed = repair(&src, &dir.path().join("fixed.pdf")).unwrap();
    render(&fixed, 0, 72).unwrap();
    thumbnail_png(&fixed, 1, 64).unwrap();
}

#[test]
fn a_healthy_field_tree_renders_flattens_and_redacts() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let src = probe14_pdf(dir.path(), "fieldchain:30");
    for page in [0, 1] {
        render(&src, page, 72).unwrap();
        thumbnail_png(&src, page, 64).unwrap();
    }
    flatten(&src, &dir.path().join("flat.pdf")).unwrap();
    redact(&src, &dir.path().join("red.pdf"), &[rd(1, 10, 10, 50, 50)]).unwrap();
    delete_annotations(&src, &dir.path().join("del.pdf"), None).unwrap();
    let discard = CompressOpts {
        discard_media: true,
        ..CompressOpts::default()
    };
    compress(&src, &dir.path().join("small.pdf"), discard).unwrap();
    // A field whose widgets are listed in /Fields' tree and on both pages.
    let form = common::fixture("annots.pdf");
    for page in [0, 1] {
        render(&form, page, 72).unwrap();
        thumbnail_png(&form, page, 64).unwrap();
    }
    flatten(&form, &dir.path().join("flat2.pdf")).unwrap();
}

#[test]
fn a_rewritten_file_is_checked_again_and_a_password_reaches_the_check() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let (ok, dag) = (dir.path().join("ok"), dir.path().join("dag"));
    std::fs::create_dir_all(&ok).unwrap();
    std::fs::create_dir_all(&dag).unwrap();
    let src = probe14_pdf(&ok, "fieldchain:3");
    render(&src, 0, 72).unwrap();
    std::fs::copy(probe14_pdf(&dag, "fielddag:3"), &src).unwrap();
    let e = render(&src, 0, 72).unwrap_err();
    assert_eq!(e.to_string(), REVISIT, "the passed file changed");
    // Opened with its user password only: the check reads it with it.
    let enc = dir.path().join("enc.pdf");
    let (a, b) = (src.to_str().unwrap(), enc.to_str().unwrap());
    common::docs::qpdf(&[a, "--encrypt", "user", "owner", "256", "--", b]);
    let pw = Secret::from("user");
    let e = thumbnail_png_with_password(&enc, 0, 64, Some(&pw)).unwrap_err();
    assert_eq!(e.to_string(), REVISIT);
    let e = thumbnail_png_with_password(&enc, 0, 64, None).unwrap_err();
    assert_eq!(e.to_string(), "encrypted input: a password is required");
    // A healthy encrypted form: its rewrite keeps the encryption (R118).
    let good = probe14_pdf(&dag, "fieldchain:3");
    let (a, b) = (good.to_str().unwrap(), enc.to_str().unwrap());
    common::docs::qpdf(&[a, "--encrypt", "user", "owner", "256", "--", b]);
    thumbnail_png_with_password(&enc, 0, 64, Some(&pw)).unwrap();
    let bad = Secret::from("nope");
    let e = thumbnail_png_with_password(&enc, 0, 64, Some(&bad)).unwrap_err();
    assert_eq!(e.to_string(), "wrong password");
}
