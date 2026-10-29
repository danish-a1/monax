//! gen8 / gen9 redaction probes (R104, R106): kept layer groups lose every
//! key but type, name and intent, and kept pages still show or hide exactly
//! what they did; many groups cost linear time.
mod common;
use common::docs::{get, objects};
use common::leaky::survivors;
use common::probe8::{GEN8, groups_pdf, probe8_pdf};
use common::secret::rd;
use monax_core::pdfium;
use monax_core::security::redact_with_notes;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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

const SECRETS: [&str; 6] = [
    "PAGESECRET",
    "USAGESECRET",
    "PRIVSECRET",
    "LBLSECRET",
    "SUBSECRET",
    "NAME75SECRET",
];

#[test]
fn kept_layers_show_and_hide_what_they_did_and_carry_nothing_else() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let mut bad = Vec::new();
    for v in GEN8 {
        let src = probe8_pdf(dir.path(), v);
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
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn a_kept_group_keeps_only_its_type_name_and_intent() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let out = redact1(&probe8_pdf(dir.path(), "usage")).unwrap();
    let o = objects(&out);
    let ocp = &get(&o, &o["trailer"]["value"]["/Root"])["/OCProperties"];
    let g = get(&o, &get(&o, ocp)["/OCGs"][0]);
    let want =
        serde_json::json!({ "/Type": "/OCG", "/Name": "u:L", "/Intent": ["/View", "/Design"] });
    assert_eq!(g, &want);
}

#[test]
fn many_groups_listed_everywhere_take_linear_time() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    // R115 draws kept page 0 twice, and pdfium checks a group's state
    // against /ON and /OFF in time quadratic in their length (20000: 12 s
    // per draw): 5000 keeps the draws short; the 60000-group walk of the
    // layers is a unit test (`layers_tests`).
    let src = groups_pdf(dir.path(), 5_000, true);
    let t = Instant::now();
    redact1(&src).unwrap();
    assert!(t.elapsed() < Duration::from_secs(15), "{:?}", t.elapsed());
}
