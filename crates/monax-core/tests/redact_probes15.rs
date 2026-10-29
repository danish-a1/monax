//! gen15 redaction probes (R112): input qpdf warns on is refused whole,
//! however many warnings bury the reason; once repaired (which notes the
//! file may display differently) it redacts without revealing; dangling or null /OCGs entries on a clean file are ignored,
//! as pdfium does.
mod common;
use common::leaky::survivors;
use common::probe15::{CLEAN, WARNED, probe15_pdf};
use common::secret::rd;
use monax_core::content::repair_with_notes;
use monax_core::pages::PagesError;
use monax_core::pdfium;
use monax_core::security::redact;
use std::path::{Path, PathBuf};

/// Dark pixels of page 0 at 72 dpi.
fn ink(p: &Path) -> usize {
    let r = pdfium::render(p, 0, 72).unwrap();
    r.rgba.chunks(4).filter(|px| px[0] < 128).count()
}

/// Redacts page 1 of `src`; on success, what went wrong with the output
/// (a secret left, page 0 drawn differently, `qpdf --check` failing).
fn redacted(src: &Path) -> Result<Vec<String>, PagesError> {
    let out = src.with_extension("out.pdf");
    let r = redact(src, &out, &[rd(1, 10, 10, 50, 50)]);
    if r.is_err() {
        assert!(!out.exists(), "{}: partial output", src.display());
    }
    let out = r?;
    let mut bad: Vec<String> = survivors(&out, &["PAGESECRET"]);
    let (a, b) = (ink(src), ink(&out));
    if a != b {
        bad.push(format!("ink {a} -> {b}"));
    }
    common::docs::assert_clean(&out);
    Ok(bad)
}

fn probes(dir: &Path, list: &str) -> Vec<PathBuf> {
    list.split_whitespace()
        .map(|v| probe15_pdf(dir, v))
        .collect()
}

#[test]
fn input_qpdf_warns_on_is_refused_and_redacts_once_repaired() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let mut bad = Vec::new();
    for src in probes(dir.path(), WARNED) {
        let v = src.file_stem().unwrap().to_string_lossy().into_owned();
        match redacted(&src) {
            Err(e @ PagesError::DamagedRepairFirst) => {
                assert_eq!(e.to_string(), "damaged file: repair it first, then redact");
            }
            r => bad.push(format!("{v}: not refused: {r:?}")),
        }
        let (fixed, notes) = repair_with_notes(&src, &src.with_extension("fixed.pdf")).unwrap();
        // R114: repair shows qpdf's reading, and says so.
        let want = ["repaired file may display differently from the damaged original"];
        assert_eq!(notes, want);
        match redacted(&fixed) {
            Ok(b) if b.is_empty() => {}
            r => bad.push(format!("{v} repaired: {r:?}")),
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn null_or_dangling_layers_on_a_clean_file_are_ignored() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let mut bad = Vec::new();
    for src in probes(dir.path(), CLEAN) {
        let v = src.file_stem().unwrap().to_string_lossy().into_owned();
        match redacted(&src) {
            Ok(b) if b.is_empty() => {}
            r => bad.push(format!("{v}: {r:?}")),
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}
