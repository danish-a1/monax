//! gen6 redaction probes (R104): kept pages keep what they need even when
//! it also names the redacted page - containers are rewritten, never
//! nulled whole; hidden layers stay hidden; a lost form font is noted.
mod common;
use common::leaky::survivors;
use common::probe6::probe6_pdf;
use common::secret::{qdf, rd};
use monax_core::pdfium;
use monax_core::security::redact_with_notes;
use std::path::{Path, PathBuf};

const LAYERS: &str = "layers used only by redacted pages were removed";
const FONTS: &str = "form fonts shared with redacted pages were removed; kept fields may show their values in another font";

fn redact1(src: &Path) -> Result<(PathBuf, Vec<String>), String> {
    let name = src.file_stem().unwrap().to_string_lossy();
    let out = src.with_file_name(format!("{name}-out.pdf"));
    let r = redact_with_notes(src, &out, &[rd(1, 10, 10, 50, 50)]);
    r.map_err(|e| format!("{name}: {e}"))
}

/// Dark pixels of page 0 at 72 dpi.
fn ink(p: &Path) -> usize {
    let r = pdfium::render(p, 0, 72).unwrap();
    r.rgba.chunks(4).filter(|px| px[0] < 128).count()
}

#[test]
fn kept_pages_keep_what_also_names_the_redacted_page_and_nothing_leaks() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    // (probe, text that must stay, secrets that must go)
    let cases: [(&str, &[&str], &[&str]); 9] = [
        ("kidsind", &[], &[]),
        ("annotsind", &["KEPTVALUE", "/AcroForm"], &[]),
        ("fieldsred", &["KEPTVALUE"], &[]),
        ("redlink", &[], &["PIECESECRET"]),
        ("actnext", &[], &["ACTSECRET"]),
        ("xodesc", &["KEEPME"], &[]),
        (
            "redwidgetfield",
            &["SHAREDVAL"],
            &["MKSECRET", "TUSECRET", "APSECRET"],
        ),
        ("drpage", &["KEPTVALUE"], &["PIECESECRET", "PAGEDICTSECRET"]),
        ("keptpieceref", &[], &["PAGEDICTSECRET"]),
    ];
    let mut bad = Vec::new();
    for (v, kept, gone) in cases {
        let src = probe6_pdf(dir.path(), v);
        let out = match redact1(&src) {
            Ok((out, _)) => out,
            Err(e) => {
                bad.push(e);
                continue;
            }
        };
        let all = qdf(&out);
        let lost: Vec<&&str> = kept.iter().filter(|k| !all.contains(**k)).collect();
        let left = survivors(&out, &[gone, &["PAGESECRET"]].concat());
        if !lost.is_empty() || !left.is_empty() {
            bad.push(format!("{v}: lost {lost:?}, survived {left:?}"));
        }
        common::docs::assert_clean(&out);
    }
    let x = dir.path().join("xodesc-out.pdf");
    if x.exists() && !pdfium::extract_text(&x, 0..=0).unwrap().contains("KEEPME") {
        bad.push("xodesc: page 0 no longer shows its form".into());
    }
    let a = dir.path().join("annotsind-out.pdf");
    if a.exists() && common::docs::annots(&a)[0] != ["/Widget", "/Link"] {
        bad.push(format!(
            "annotsind: page 0 annotations {:?}",
            common::docs::annots(&a)[0]
        ));
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn a_layer_off_on_a_kept_page_stays_off() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let src = probe6_pdf(dir.path(), "hiddenocg");
    assert_eq!(ink(&src), 0, "fixture: the layer is off");
    let (out, notes) = redact1(&src).unwrap();
    assert_eq!(ink(&out), 0, "hidden content revealed");
    assert!(!notes.iter().any(|n| n == LAYERS), "{notes:?}");
    common::docs::assert_clean(&out);
}

#[test]
fn a_kept_field_losing_a_shared_form_font_is_refused() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    // R115: the kept field would draw without its form font: refused.
    let e = redact1(&probe6_pdf(dir.path(), "drfont")).unwrap_err();
    assert_eq!(
        e,
        "drfont: redaction check failed: a kept page would change"
    );
    let (_, notes) = redact1(&probe6_pdf(dir.path(), "annotsind")).unwrap();
    assert!(!notes.iter().any(|n| n == FONTS), "{notes:?}");
}
