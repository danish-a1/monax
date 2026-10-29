//! Images to PDF as a job: every input is checked before any image is
//! decoded, progress per image, and a cancel publishes nothing.
mod common;
use common::convert::{cancelled, idle};
use common::names;
use common::sign::{RED, png};
use monax_core::convert::{ImagesToPdfInput, images_to_pdf};
use monax_core::pages::PagesError;
use serde_json::json;
use std::path::PathBuf;

fn invalid(r: Result<PathBuf, PagesError>) -> String {
    match r {
        Err(PagesError::InvalidInput(m)) => m,
        other => panic!("expected InvalidInput, got {other:?}"),
    }
}

#[test]
fn bad_inputs_are_found_before_any_image_is_decoded() {
    require_qpdf!();
    let d = tempfile::tempdir().unwrap();
    // A PNG cut after its header (only decoding finds it) before a GIF.
    let cut = png(d.path(), "cut.png", (8, 8), |_, _| RED);
    std::fs::write(&cut, &std::fs::read(&cut).unwrap()[..40]).unwrap();
    let gif = d.path().join("x.gif");
    std::fs::write(&gif, b"GIF89a").unwrap();
    let r = images_to_pdf(&[cut, gif.clone()], &d.path().join("o.pdf"), None, &idle());
    assert_eq!(
        invalid(r),
        format!("image {} is not a PNG or JPEG", gif.display())
    );
    // A PDF given as the only image: the image is wrong, not the output.
    let pdf = d.path().join("doc.pdf");
    std::fs::write(&pdf, b"%PDF-1.4").unwrap();
    let r = images_to_pdf(std::slice::from_ref(&pdf), &pdf, None, &idle());
    assert_eq!(
        invalid(r),
        format!("image {} is not a PNG or JPEG", pdf.display())
    );
    // The default output of a bad first image reports the image.
    for (first, want) in [
        ("rel.png", "path must be absolute: rel.png".to_string()),
        (
            "/no/such/dir/a.png",
            "file does not exist: /no/such/dir/a.png".to_string(),
        ),
    ] {
        let v = json!({ "inputs": [first] });
        let r = serde_json::from_value::<ImagesToPdfInput>(v)
            .unwrap()
            .run(&idle());
        assert_eq!(invalid(r), want);
    }
    assert_eq!(names(d.path()), ["cut.png", "doc.pdf", "x.gif"]);
}

#[test]
fn progress_per_image_and_cancel_leaves_nothing() {
    require_qpdf!();
    let d = tempfile::tempdir().unwrap();
    let a = png(d.path(), "a.png", (4, 4), |_, _| RED);
    let out = d.path().join("o.pdf");
    let r = images_to_pdf(&[a.clone(), a.clone()], &out, None, &cancelled());
    assert!(matches!(r, Err(PagesError::Cancelled)), "{r:?}");
    assert_eq!(names(d.path()), ["a.png"]);
    // Two images: 1/3 and 2/3 before the join (the job ends at 1).
    let reg = monax_core::Registry::new();
    let (ids, id_rx) = std::sync::mpsc::channel();
    let (tx, rx) = std::sync::mpsc::channel();
    let r2 = reg.clone();
    let job = reg.submit(move |s| {
        let id = id_rx.recv().unwrap();
        let r = images_to_pdf(&[a.clone(), a], &out, None, &s);
        tx.send((r.is_ok(), r2.status(id).unwrap().progress))
            .unwrap();
        Ok::<_, String>(vec![])
    });
    ids.send(job).unwrap();
    let (ok, progress) = rx.recv().unwrap();
    assert!(ok && (progress - 2.0 / 3.0).abs() < 1e-6, "{progress}");
}
