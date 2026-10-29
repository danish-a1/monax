//! Office to PDF through a fake `wordcraft-cli`: argv, output naming, type
//! checks, no output, failure, cancel of the whole process group, the input
//! struct, and no temp dir left on any path. (The 300 s bound is a unit
//! test; the runner's timeout kill of wordcraft-cli is in monax-sidecar.)
mod common;
use common::names;
use common::ocr::job;
use common::office::{argv, fake, gone, pid, temp};
use monax_core::convert::{OfficeToPdfInput, office_available, office_to_pdf};
use monax_core::pages::PagesError;
use monax_core::{JobState, Registry};
use serde_json::json;
use std::path::{Path, PathBuf};

fn to_pdf(input: &Path, dir: &Path) -> (Result<PathBuf, PagesError>, Vec<String>) {
    let (input, dir) = (input.to_path_buf(), dir.to_path_buf());
    let (r, j) = job(move |s| office_to_pdf(&input, &dir, s));
    (r, j.notes)
}

fn invalid(r: Result<PathBuf, PagesError>) -> String {
    match r {
        Err(PagesError::InvalidInput(m)) => m,
        r => panic!("expected InvalidInput, got {r:?}"),
    }
}

/// The run's private temp dir is gone.
fn cleaned(log: &Path) {
    let t = temp(log);
    assert!(!t.exists(), "{t:?} left behind");
}

// Every scenario mutates process env, so they share one test to avoid races.
#[test]
fn office_to_pdf_through_fake_wordcraft() {
    let d = tempfile::tempdir().unwrap();
    let logs = tempfile::tempdir().unwrap();
    let log = logs.path().join("wordcraft.log");
    fake("ok", &log);
    let bin = common::fixture("bin-wordcraft/wordcraft-cli");
    assert_eq!(office_available(), Some(bin));

    let doc = d.path().join("Report.DOCX");
    std::fs::write(&doc, b"PK\x03\x04 docx bytes").unwrap();
    let (r, notes) = to_pdf(&doc, d.path());
    assert_eq!(r.unwrap(), d.path().join("Report.pdf"));
    assert!(notes.is_empty(), "{notes:?}");
    // `convert <input> <private dir>/out.pdf`: the input read in place,
    // the output written only into a private 0700 dir.
    let got = argv(&log);
    let made = temp(&log).join("out.pdf");
    assert_eq!(
        got,
        ["convert", &*doc.to_string_lossy(), &*made.to_string_lossy()]
    );
    assert!(!made.starts_with(d.path()));
    let out = std::fs::read(d.path().join("Report.pdf")).unwrap();
    assert_eq!(out, b"converted\nPK\x03\x04 docx bytes");
    cleaned(&log);

    // Copy-on-save: a taken name gets -1; the input is untouched.
    let r = to_pdf(&doc, d.path()).0;
    assert_eq!(r.unwrap(), d.path().join("Report-1.pdf"));
    assert_eq!(std::fs::read(&doc).unwrap(), b"PK\x03\x04 docx bytes");

    // The input struct: default dir beside the input, unknown fields refused.
    let md = d.path().join("notes.md");
    std::fs::write(&md, b"# Notes").unwrap();
    let i: OfficeToPdfInput = serde_json::from_value(json!({ "input": md })).unwrap();
    assert_eq!(
        job(move |s| i.run(s)).0.unwrap(),
        d.path().join("notes.pdf")
    );
    let i = serde_json::from_value::<OfficeToPdfInput>(json!({ "input": md, "x": 1 }));
    assert!(i.is_err());

    // Type checks run before wordcraft-cli: nothing is spawned.
    std::fs::remove_file(&log).unwrap();
    let sheet = d.path().join("a.xlsx");
    std::fs::write(&sheet, b"PK\x03\x04").unwrap();
    let list = "docx, doc, odt, rtf, txt, html, htm, md";
    let want = format!(
        "{} is not a supported Office document ({list})",
        sheet.display()
    );
    assert_eq!(invalid(to_pdf(&sheet, d.path()).0), want);
    let fake_doc = d.path().join("b.doc");
    std::fs::write(&fake_doc, b"{\\rtf1 hi}").unwrap();
    let want = format!("{} is not a valid .doc file", fake_doc.display());
    assert_eq!(invalid(to_pdf(&fake_doc, d.path()).0), want);
    let locked = d.path().join("locked.docx");
    std::fs::write(&locked, b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1").unwrap();
    let r = to_pdf(&locked, d.path()).0;
    assert!(matches!(r, Err(PagesError::EncryptedInput)), "{r:?}");
    let m = invalid(to_pdf(Path::new("rel.docx"), d.path()).0);
    assert_eq!(m, "path must be absolute: rel.docx");
    let m = invalid(to_pdf(&doc, Path::new("out")).0);
    assert!(
        m.starts_with("output directory must be an absolute path"),
        "{m}"
    );
    assert!(!log.exists(), "wordcraft-cli ran for a refused input");

    let before = names(d.path());
    // Exit 0 without a file is an error, never success.
    fake("none", &log);
    let r = to_pdf(&doc, d.path()).0;
    assert!(matches!(r, Err(PagesError::OfficeNoOutput)), "{r:?}");
    assert_eq!(r.unwrap_err().to_string(), "WordCraft produced no output");
    cleaned(&log);

    // A non-zero exit carries the last non-empty stderr line, at most 200 chars.
    fake("fail", &log);
    let m = to_pdf(&doc, d.path()).0.unwrap_err().to_string();
    assert_eq!(m, "WordCraft failed: wordcraft-cli: not a Word document");
    cleaned(&log);
    // SAFETY: the only test in this binary.
    unsafe { std::env::set_var("FAKE_WORDCRAFT_ERR", "é".repeat(300)) };
    let m = to_pdf(&doc, d.path()).0.unwrap_err().to_string();
    assert_eq!(m, format!("WordCraft failed: {}", "é".repeat(200)));
    cleaned(&log);
    // WordCraft's password-required error (an encrypted .doc) is EncryptedInput.
    let err = "wordcraft-cli: a.doc: this document is protected with a password";
    // SAFETY: the only test in this binary.
    unsafe { std::env::set_var("FAKE_WORDCRAFT_ERR", err) };
    let r = to_pdf(&doc, d.path()).0;
    assert!(matches!(r, Err(PagesError::EncryptedInput)), "{r:?}");
    cleaned(&log);

    // Cancel kills wordcraft-cli and the child it forked; nothing is written.
    fake("hang", &log);
    let reg = Registry::new();
    let (input, dir) = (doc.clone(), d.path().to_path_buf());
    let id = reg.submit(move |s| office_to_pdf(&input, &dir, &s).map(|p| vec![p]));
    let child = pid(&log);
    reg.cancel(id);
    let j = reg.wait(id).unwrap();
    assert!(matches!(j.state, JobState::Cancelled), "{:?}", j.state);
    assert!(gone(child), "wordcraft-cli's child outlived the cancel");
    cleaned(&log);
    assert_eq!(names(d.path()), before);
}
