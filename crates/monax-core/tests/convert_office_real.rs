//! Real WordCraft, when built into `sidecars/` (or `MONAX_BIN_DIR`): one
//! tiny checked-in document per family becomes a PDF of 1+ pages holding
//! its text. Skips with a notice otherwise. (No `.doc`: an OLE Word file
//! cannot be made by hand at fixture size; its magic check is unit-tested.)
mod common;
use common::convert::idle;
use monax_core::convert::office_to_pdf;
use monax_core::pdfium;

#[test]
fn real_wordcraft_converts_each_family() {
    require_pdfium!();
    if common::office::real().is_none() {
        return;
    }
    let d = tempfile::tempdir().unwrap();
    for ext in ["docx", "odt", "rtf", "txt", "md", "html", "DOCX", "htm"] {
        let input = d.path().join(format!("in/hello.{ext}"));
        let src = common::fixture(&format!("office/hello.{}", fixture_ext(ext)));
        std::fs::create_dir_all(input.parent().unwrap()).unwrap();
        std::fs::copy(src, &input).unwrap();
        let out = office_to_pdf(&input, d.path(), &idle()).unwrap();
        assert_eq!(out, d.path().join("hello.pdf"));
        let n = pdfium::page_count(&out).unwrap();
        assert!(n >= 1, "{ext}: {n} pages");
        let text = pdfium::extract_text(&out, 0..=n - 1).unwrap();
        assert!(text.contains("Quokka"), "{ext}: {text:?}");
        std::fs::remove_file(&out).unwrap();
    }
}

/// The checked-in fixture an extension case reads: `.DOCX` and `.htm` reuse
/// the docx and html files.
fn fixture_ext(ext: &str) -> &str {
    match ext {
        "DOCX" => "docx",
        "htm" => "html",
        e => e,
    }
}
