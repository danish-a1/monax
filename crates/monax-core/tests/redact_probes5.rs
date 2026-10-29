//! gen3 / gen4 / gen5 redaction probes: no document-level root, private
//! key or kept reference rescues what only a redacted page owns; every
//! document-level feature is dropped and named; what kept pages share stays;
//! Type3 cycles and deep form chains do not crash the leak check.
mod common;
use common::docfeat::{FEATURES, feature_pdf};
use common::docs::{get, objects};
use common::leaky::survivors;
use common::probe3::probe3_pdf;
use common::probe4::probe4_pdf;
use common::probe5::probe5_pdf;
use common::secret::{qdf, rd};
use monax_core::pdfium;
use monax_core::security::redact_with_notes;
use std::path::{Path, PathBuf};

const DROPPED: &str = "document-level features were removed: ";

type Run = Result<(PathBuf, Vec<String>), String>;

fn redact1(src: &Path) -> Run {
    let name = src.file_stem().unwrap().to_string_lossy();
    let out = src.with_file_name(format!("{name}-out.pdf"));
    let r = redact_with_notes(src, &out, &[rd(1, 10, 10, 50, 50)]);
    r.map_err(|e| format!("{name}: {e}"))
}

/// Runs every case and collects what went wrong: an error, a survivor, a
/// `qpdf --check` complaint.
fn gone(cases: &[(PathBuf, &[&str])]) -> Vec<String> {
    let mut bad = Vec::new();
    for (src, secrets) in cases {
        match redact1(src) {
            Ok((out, _)) => {
                let left = survivors(&out, &[*secrets, &["PAGESECRET"]].concat());
                if !left.is_empty() {
                    bad.push(format!("{}: {left:?}", src.display()));
                }
                common::docs::assert_clean(&out);
            }
            Err(e) => bad.push(e),
        }
    }
    bad
}

#[test]
fn no_document_root_or_private_key_rescues_a_redacted_page() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let p5: [(&str, &[&str]); 11] = [
        ("ocpcontent", &[]),
        ("drcontent", &[]),
        ("drcontent2", &[]),
        ("drform", &["FORMSECRET"]),
        ("pkey", &["PIECESECRET"]),
        ("parentkey", &["PIECESECRET"]),
        ("ocppiece", &["PIECESECRET"]),
        ("unusedres", &["FORMSECRET"]),
        ("unusedimg", &["IMAGESECRET"]),
        ("popupback", &["NOTESECRET"]),
        ("optfirst", &["OPTSECRET"]),
    ];
    let p3: [(&str, &[&str]); 4] = [
        ("pref", &[]),
        ("catref", &["CATSECRET", "OUTLINESECRET"]),
        ("describers", &["ALTSECRET", "OUTLINESECRET", "ACTSECRET"]),
        ("attach", &["ATTSECRET", "ATTNOTE"]),
    ];
    let p4: [(&str, &[&str]); 4] = [
        ("piecelocal", &["PIECESECRET"]),
        ("meta", &["METASECRET"]),
        ("tu", &["TUSECRET"]),
        ("aa", &["JSSECRET", "JSSTREAMSECRET"]),
    ];
    let mut cases: Vec<(PathBuf, &[&str])> =
        p5.iter().map(|(v, s)| (probe5_pdf(d, v, 0), *s)).collect();
    cases.extend(p3.iter().map(|(v, s)| (probe3_pdf(d, v), *s)));
    cases.extend(p4.iter().map(|(v, s)| (probe4_pdf(d, v), *s)));
    let bad = gone(&cases);
    assert!(bad.is_empty(), "survived: {bad:?}");
}

#[test]
fn every_dropped_document_feature_is_named() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let mut bad = Vec::new();
    for (v, secret, label) in FEATURES {
        let src = feature_pdf(dir.path(), v);
        assert!(qdf(&src).contains(secret), "{v}: fixture");
        match redact1(&src) {
            Ok((out, notes)) => {
                let left = survivors(&out, &[secret, "PAGESECRET"]);
                let note = notes.iter().find_map(|n| n.strip_prefix(DROPPED));
                let named = note.is_some_and(|n| n.split(", ").any(|l| l == label));
                if !left.is_empty() || !named {
                    bad.push(format!("{v}: survived {left:?}, notes {notes:?}"));
                }
                common::docs::assert_clean(&out);
            }
            Err(e) => bad.push(e),
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn what_kept_pages_share_stays() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let mut bad = Vec::new();
    // (probe, kept text, text that must go)
    let d = dir.path();
    let cases = [
        (probe3_pdf(d, "sharedform"), "SHAREDTEXT", "PAGESECRET"),
        (probe3_pdf(d, "apshared"), "APTEXT", "FTSECRET"),
        // The kept stamp's nested form says a word of page 1 (N5).
        (probe5_pdf(d, "frm", 0), "PAGESECRET", "NOSUCHTEXT"),
    ];
    for (src, kept, lost) in cases {
        let v = src.file_stem().unwrap().to_string_lossy().into_owned();
        match redact1(&src) {
            Ok((out, _)) => {
                let all = qdf(&out);
                if !all.contains(kept) || !survivors(&out, &[lost]).is_empty() {
                    bad.push(format!("{v}: {kept} lost or {lost} kept"));
                }
                common::docs::assert_clean(&out);
            }
            Err(e) => bad.push(e),
        }
    }
    let out = dir.path().join("sharedform-out.pdf");
    if out.exists()
        && !pdfium::extract_text(&out, 0..=0)
            .unwrap()
            .contains("SHAREDTEXT")
    {
        bad.push("sharedform: page 0 no longer shows its form".into());
    }
    assert!(bad.is_empty(), "{bad:?}");
}

#[test]
fn a_kept_field_keeps_its_font() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let (out, _) = redact1(&probe5_pdf(dir.path(), "drcontent2", 0)).unwrap();
    let o = objects(&out);
    let cat = get(&o, &o["trailer"]["value"]["/Root"]);
    let dr = get(&o, &get(&o, &cat["/AcroForm"])["/DR"]);
    assert_eq!(get(&o, &dr["/Font"]["/F1"])["/BaseFont"], "/Helvetica");
    assert!(qdf(&out).contains("(hi)"), "the kept field keeps its value");
}

#[test]
fn a_kept_radio_keeps_its_export_value_when_an_earlier_kid_goes() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let (out, _) = redact1(&probe5_pdf(dir.path(), "optfirst", 0)).unwrap();
    let all = qdf(&out);
    let opt = all.split("/Opt [").nth(1).unwrap_or_default();
    let opt: Vec<&str> = opt.split(']').next().unwrap().split_whitespace().collect();
    assert_eq!(opt, ["()", "(Healthy)"], "/V /1 still names Healthy");
    assert!(all.contains("/V /1"));
    common::docs::assert_clean(&out);
}

#[test]
fn type3_cycles_and_deep_form_chains_do_not_crash() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let mut bad = Vec::new();
    for (v, n) in [("t3cycle", 0), ("t3pair", 0), ("t3used", 0), ("deep", 3000)] {
        match redact1(&probe5_pdf(dir.path(), v, n)) {
            Ok((out, _)) => common::docs::assert_clean(&out),
            Err(e) => bad.push(e),
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}
