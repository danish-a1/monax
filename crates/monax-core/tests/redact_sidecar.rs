//! redact builds its output in one qpdf pass over all its pages, and a failing or missing qpdf
//! is reported with the exact reason and no output.
mod common;
use common::{fixture, names, setup};
use monax_core::pages::{PagesError, Rect};
use monax_core::security::{Redaction, redact};
use monax_sidecar::SidecarError;

// Every step mutates process env, so this binary has one test.
#[test]
fn one_qpdf_pass_and_sidecar_failures_leave_no_output() {
    require_qpdf!();
    let real = monax_sidecar::find(monax_sidecar::Tool::Qpdf).unwrap();
    let (dir, three) = setup();
    let record = dir.path().join("record.txt");
    // SAFETY: the only test in this binary.
    unsafe {
        std::env::set_var("MONAX_REAL_QPDF", &real);
        std::env::set_var("MONAX_QPDF_RECORD", &record);
        std::env::set_var("MONAX_BIN_DIR", fixture("bin-record"));
    }
    let rect = Rect {
        left: 0.0,
        bottom: 0.0,
        right: 50.0,
        top: 50.0,
    };
    let boxes: Vec<Redaction> = [0, 2, 0].map(|page| Redaction { page, rect }).into();
    redact(&three, &dir.path().join("o.pdf"), &boxes).unwrap();
    let log = std::fs::read_to_string(&record).unwrap();
    assert_eq!(log.matches("arg --pages\n").count(), 1, "{log}");
    // Built from the staged copy (its catalog cut to pages, form and kept
    // layers) and the pages alone (R103, R104): kept page 1 from it.
    assert_eq!(log.matches("arg .\n").count(), 1, "{log}");
    // The job's own files (staged copy, raster, update, read-back JSON):
    // private (0600 in a 0700 dir), removed afterwards.
    let ours: Vec<(&str, &str, &str)> = log
        .lines()
        .filter_map(|l| {
            let (mode, rest) = l.strip_prefix("mode ")?.split_once(' ')?;
            let (dmode, path) = rest.split_once(' ')?;
            Some((mode, dmode, path))
        })
        .filter(|(_, _, p)| !p.starts_with(&*dir.path().to_string_lossy()))
        .collect();
    assert!(ours.len() >= 3, "{ours:?}");
    for (mode, dmode, path) in ours {
        assert_eq!((mode, dmode), ("600", "700"), "{path}");
        assert!(!std::path::Path::new(path).exists(), "left behind: {path}");
    }
    std::fs::remove_file(&record).unwrap();

    let fail = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../monax-sidecar/tests/fixtures/bin-fail"
    );
    // SAFETY: as above.
    unsafe { std::env::set_var("MONAX_BIN_DIR", fail) };
    let out = dir.path().join("f.pdf");
    let r = redact(&three, &out, &boxes);
    assert!(
        matches!(&r, Err(PagesError::Qpdf(m)) if m == "exit code 1"),
        "{r:?}"
    );
    // SAFETY: as above.
    unsafe {
        std::env::set_var("MONAX_BIN_DIR", "/nonexistent");
        std::env::remove_var("PATH");
    }
    let r = redact(&three, &out, &boxes);
    let missing = matches!(r, Err(PagesError::Sidecar(SidecarError::NotFound("qpdf"))));
    assert!(missing, "{r:?}");
    assert_eq!(names(dir.path()), ["o.pdf", "three.pdf"]);
}
