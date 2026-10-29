//! gen16 redaction probes (R115): qpdf reads them clean but other than
//! pdfium, so the output would draw a kept page differently; the final
//! gate refuses it whole, nothing written.
mod common;
use common::probe16::{GEN16, probe16_pdf};
use common::secret::rd;
use monax_core::security::redact;

fn refused(form: &str) {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let mut bad = Vec::new();
    for v in GEN16.split_whitespace().map(|v| format!("{v}{form}")) {
        let v = v.as_str();
        let src = probe16_pdf(dir.path(), v);
        let out = dir.path().join(format!("{v}-out.pdf"));
        match redact(&src, &out, &[rd(1, 10, 10, 50, 50)]) {
            Err(e) if e.to_string() == "redaction check failed: a kept page would change" => {}
            r => bad.push(format!("{v}: {r:?}")),
        }
        if out.exists() {
            bad.push(format!("{v}: output written"));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn a_kept_page_that_would_change_is_refused() {
    refused("");
}

// R119: redaction draws the original form, not qpdf's rewrite of it.
#[test]
fn a_kept_page_of_a_form_that_would_change_is_refused() {
    refused("+form");
}
