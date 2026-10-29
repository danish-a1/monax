//! redact refuses bad boxes and pages too large to rasterize, before
//! rendering anything and without leaving a file.
mod common;
use common::names;
use common::secret::rd;
use monax_core::pages::PagesError;
use monax_core::security::redact;

#[test]
fn bad_redactions_fail_without_output() {
    require_qpdf!();
    let (dir, src) = common::setup();
    let o = dir.path().join("o.pdf");
    let msg = |r: Result<_, PagesError>| r.map(drop).unwrap_err().to_string();
    assert_eq!(
        msg(redact(&src, &o, &[])),
        "invalid input: no redactions given"
    );
    assert_eq!(
        msg(redact(&src, &o, &[rd(5, 0, 0, 1, 1)])),
        "page index 5 out of range (document has 3 pages, indices 0-2)"
    );
    let mut nan = rd(0, 0, 0, 1, 1);
    nan.rect.left = f32::NAN;
    for r in [rd(0, 10, 10, 10, 20), rd(0, 10, 20, 20, 10), nan] {
        assert_eq!(
            msg(redact(&src, &o, &[r])),
            "invalid input: redaction rect on page 0 is empty"
        );
    }
    assert_eq!(
        msg(redact(
            &src,
            &o,
            &[rd(0, 0, 0, 1, 1), rd(1, 200, 0, 300, 100)]
        )),
        "invalid input: redaction rect on page 1 is outside the page"
    );
    assert_eq!(names(dir.path()), ["three.pdf"]);
}

#[test]
fn pages_too_large_for_a_300_dpi_raster_fail_before_rendering() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let cases = [
        (
            "2000 2000",
            "invalid input: 8334x8334 px exceeds the 64000000 pixel budget",
        ),
        ("16000 10", "invalid input: page 0 is too large to redact"),
    ];
    for (size, want) in cases {
        let page = format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {size}] >>");
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            &page,
        ];
        let objs: Vec<Vec<u8>> = objs.iter().map(|o| o.as_bytes().to_vec()).collect();
        let big = common::scan::write_pdf(dir.path(), "big.pdf", &objs);
        let r = redact(&big, &dir.path().join("o.pdf"), &[rd(0, 0, 0, 5, 5)]);
        assert_eq!(r.unwrap_err().to_string(), want);
    }
    assert_eq!(names(dir.path()), ["big.pdf"]);
}
