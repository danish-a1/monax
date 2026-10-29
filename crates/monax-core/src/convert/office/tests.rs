//! Allow-list and magic checks of Office to PDF (no sidecar runs).
use super::{TIMEOUT, check_type, starts_as};
use crate::pages::PagesError;
use std::path::Path;

const PK: &[u8] = b"PK\x03\x04";
const OLE: &[u8] = b"\xD0\xCF\x11\xE0";
const RTF: &[u8] = b"{\\rtf1";

#[test]
fn each_type_starts_with_its_own_signature() {
    for ext in ["docx", "odt"] {
        assert!(starts_as(ext, PK), "{ext}");
        assert!(!starts_as(ext, OLE) && !starts_as(ext, RTF) && !starts_as(ext, b""));
    }
    assert!(starts_as("doc", OLE));
    assert!(
        !starts_as("doc", PK) && !starts_as("doc", RTF),
        "doc is OLE only"
    );
    assert!(starts_as("rtf", RTF) && !starts_as("rtf", b"{\\rt") && !starts_as("rtf", PK));
    for ext in ["txt", "html", "htm", "md"] {
        assert!(starts_as(ext, b"") && starts_as(ext, b"%PDF-") && starts_as(ext, PK));
    }
}

fn check(name: &str, bytes: &[u8]) -> Result<(), String> {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join(name);
    std::fs::write(&p, bytes).unwrap();
    check_type(&p).map_err(|e| match e {
        PagesError::InvalidInput(m) => m.replace(&*d.path().to_string_lossy(), ""),
        e => panic!("expected InvalidInput, got {e:?}"),
    })
}

#[test]
fn the_allow_list_is_case_insensitive_and_refuses_sheets_slides_and_pdf() {
    for (name, bytes) in [
        ("a.DOCX", PK),
        ("a.Odt", PK),
        ("a.doc", OLE),
        ("a.RTF", RTF),
        ("a.txt", b"hi"),
        ("a.html", b"<p>hi"),
        ("a.HTM", b""),
        ("a.md", b"# hi"),
    ] {
        assert_eq!(check(name, bytes), Ok(()), "{name}");
    }
    let list = "docx, doc, odt, rtf, txt, html, htm, md";
    for name in ["s.xlsx", "p.pptx", "x.pdf", "c.csv", "o.odp", "noext"] {
        let want = format!("/{name} is not a supported Office document ({list})");
        assert_eq!(check(name, PK), Err(want));
    }
    let doc = check("b.doc", RTF);
    assert_eq!(doc, Err("/b.doc is not a valid .doc file".into()));
    assert_eq!(
        check("r.docx", b"%PDF-1.7"),
        Err("/r.docx is not a valid .docx file".into())
    );
    let rel = check_type(Path::new("rel.docx")).unwrap_err().to_string();
    assert_eq!(rel, "invalid input: path must be absolute: rel.docx");
}

#[test]
fn an_ole_headed_docx_is_encrypted_input() {
    // R155: encrypted / IRM OOXML is a CFB container, not a zip.
    let d = tempfile::tempdir().unwrap();
    for name in ["e.docx", "E.DOCX"] {
        let p = d.path().join(name);
        std::fs::write(&p, OLE).unwrap();
        let r = check_type(&p);
        assert!(
            matches!(r, Err(PagesError::EncryptedInput)),
            "{name}: {r:?}"
        );
    }
}

#[test]
fn a_conversion_is_bounded_to_300_seconds() {
    assert_eq!(TIMEOUT.as_secs(), 300);
}
