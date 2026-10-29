//! gen10 redaction probes (R104, R106): kept group dictionaries keep the
//! view state, name and intent pdfium reads, so page 0 shows and hides
//! what it did; kept non-groups listed in /OCGs are left as they are.
mod common;
use common::docs::{get, objects};
use common::leaky::survivors;
use common::probe10::{GEN10, probe10_pdf};
use common::secret::{qdf, rd};
use monax_core::pdfium;
use monax_core::security::redact_with_notes;
use std::path::{Path, PathBuf};

fn redact1(src: &Path) -> Result<PathBuf, String> {
    let name = src.file_stem().unwrap().to_string_lossy();
    let out = src.with_file_name(format!("{name}-out.pdf"));
    let r = redact_with_notes(src, &out, &[rd(1, 10, 10, 50, 50)]);
    r.map(|(o, _)| o).map_err(|e| format!("{name}: {e}"))
}

/// Dark pixels of page 0 at 72 dpi.
fn ink(p: &Path) -> usize {
    let r = pdfium::render(p, 0, 72).unwrap();
    r.rgba.chunks(4).filter(|px| px[0] < 128).count()
}

const SECRETS: [&str; 7] = [
    "PAGESECRET",
    "NAMEDICTSECRET",
    "NAMEDEEPSECRET",
    "NAMEARRSECRET",
    "NAMEARRREFSECRET",
    "INTENTSECRET",
    "TYPELESSSECRET",
];

#[test]
fn kept_groups_show_and_hide_what_they_did_and_non_groups_stay_whole() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let mut bad = Vec::new();
    for v in GEN10 {
        let src = probe10_pdf(dir.path(), v);
        let out = match redact1(&src) {
            Ok(out) => out,
            Err(e) => {
                bad.push(e);
                continue;
            }
        };
        let left = survivors(&out, &SECRETS);
        let (a, b) = (ink(&src), ink(&out));
        if !left.is_empty() || a != b {
            bad.push(format!("{v}: survived {left:?}, ink {a} -> {b}"));
        }
        common::docs::assert_clean(&out);
    }
    let res = dir.path().join("listres-out.pdf");
    if res.exists() && !qdf(&res).contains("/Properties") {
        bad.push("listres: page 0 lost its resources".into());
    }
    let kept = dir.path().join("nameref-out.pdf");
    if kept.exists() && !qdf(&kept).contains("NAMEREFKEPT") {
        bad.push("nameref: the indirect name went".into());
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn a_listed_font_stays_a_font_and_leaves_the_group_list() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let out = redact1(&probe10_pdf(dir.path(), "listfont")).unwrap();
    let o = objects(&out);
    let ocp = &get(&o, &o["trailer"]["value"]["/Root"])["/OCProperties"];
    let ocgs = get(&o, &get(&o, ocp)["/OCGs"]).as_array().unwrap().clone();
    assert_eq!(ocgs.len(), 1, "{ocgs:?}");
    assert!(qdf(&out).contains("/BaseFont /Helvetica"));
}
