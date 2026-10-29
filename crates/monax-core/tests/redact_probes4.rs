//! gen4 redaction probes: everything only page 1 owns is gone even when a
//! catalog entry names it; what a kept page uses stays.
mod common;
use common::leaky::survivors;
use common::probe4::probe4_pdf;
use common::secret::{qdf, rd};
use monax_core::pdfium;
use monax_core::security::redact_with_notes;

fn run(v: &str, dir: &tempfile::TempDir) -> Result<std::path::PathBuf, String> {
    let src = probe4_pdf(dir.path(), v);
    let r = redact_with_notes(
        &src,
        &dir.path().join(format!("{v}-out.pdf")),
        &[rd(1, 10, 10, 50, 50)],
    );
    r.map(|(p, _)| p).map_err(|e| format!("{v}: {e}"))
}

#[test]
fn what_only_a_redacted_page_owns_is_gone() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let cases: [(&str, &str); 9] = [
        ("piece", "PIECESECRET"),
        ("thumb", "THUMBSECRET"),
        ("metaref", "METASECRET"),
        ("turef", "TUSECRET"),
        ("aaref", "JSSTREAMSECRET"),
        ("catform", "FORMSECRET"),
        ("splittj", "SPLIT"),
        ("dv", "DVSECRET"),
        ("opt", "OPTSECRET"),
    ];
    let mut bad = Vec::new();
    for (v, secret) in cases {
        match run(v, &dir) {
            Ok(out) => {
                let left = survivors(&out, &[secret, "PAGESECRET"]);
                if !left.is_empty() {
                    bad.push(format!("{v}: {left:?}"));
                }
                common::docs::assert_clean(&out);
            }
            Err(e) => bad.push(e),
        }
    }
    assert!(bad.is_empty(), "survived: {bad:?}");
}

#[test]
fn what_a_kept_page_uses_stays() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let mut bad = Vec::new();
    for (v, kept) in [("contentform", "SHOWNKEPT"), ("apkept", "APKEPTTEXT")] {
        match run(v, &dir) {
            Ok(out) if qdf(&out).contains(kept) => common::docs::assert_clean(&out),
            Ok(_) => bad.push(format!("{v}: {kept} lost")),
            Err(e) => bad.push(e),
        }
    }
    let out = dir.path().join("contentform-out.pdf");
    if out.exists()
        && !pdfium::extract_text(&out, 0..=0)
            .unwrap()
            .contains("SHOWNKEPT")
    {
        bad.push("contentform: page 0 no longer shows its form".into());
    }
    match run("nref", &dir) {
        Ok(out) if qdf(&out).contains("/V /Healthy") => common::docs::assert_clean(&out),
        Ok(_) => bad.push("nref: kept radio value reset".into()),
        Err(e) => bad.push(e),
    }
    assert!(bad.is_empty(), "{bad:?}");
}
