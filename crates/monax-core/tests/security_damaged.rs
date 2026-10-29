//! A damaged encrypted file (trailer cut off, so qpdf cannot see the
//! encryption) is never processed as plain: repair refuses it with the exact
//! reason and writes nothing, instead of emitting ciphertext or dropping the
//! owner restrictions.
mod common;
use common::{fixture, names};
use monax_core::content;
use monax_core::pages::{self, PagesError};
use std::path::{Path, PathBuf};

const REASON: &str = "damaged encrypted file: repair needs the trailer to decrypt";

/// `name` cut inside its cross-reference table, written into `dir`.
fn truncated(dir: &Path, name: &str) -> PathBuf {
    let bytes = std::fs::read(fixture(name)).unwrap();
    let at = bytes.windows(5).position(|w| w == b"\nxref").unwrap() + 20;
    let p = dir.join(format!("cut-{name}"));
    std::fs::write(&p, &bytes[..at]).unwrap();
    p
}

#[test]
fn damaged_encrypted_files_are_refused_not_repaired() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    for name in ["encrypted.pdf", "owner-only.pdf"] {
        let cut = truncated(dir.path(), name);
        let out = dir.path().join("o.pdf");
        let r = content::repair(&cut, &out);
        assert!(
            matches!(r, Err(PagesError::DamagedEncrypted)),
            "{name}: {r:?}"
        );
        assert_eq!(r.unwrap_err().to_string(), REASON);
        let v = serde_json::json!({ "input": cut });
        let r = serde_json::from_value::<content::RepairInput>(v)
            .unwrap()
            .run();
        assert!(
            matches!(r, Err(PagesError::DamagedEncrypted)),
            "{name}: {r:?}"
        );
        let r = pages::rotate(&cut, &[0], 90, &out);
        assert!(r.is_err(), "{name}: {r:?}");
    }
    assert_eq!(
        names(dir.path()),
        ["cut-encrypted.pdf", "cut-owner-only.pdf"]
    );
}

#[test]
fn a_damaged_plain_file_is_still_repaired() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let cut = truncated(dir.path(), "three-pages.pdf");
    let out = content::repair(&cut, &dir.path().join("o.pdf")).unwrap();
    assert_eq!(monax_core::pdfium::page_count(&out).unwrap(), 3);
}
