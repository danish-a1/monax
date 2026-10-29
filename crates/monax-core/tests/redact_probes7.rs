//! gen7 redaction probes (R104): layer properties are rebuilt from the kept
//! groups alone - nothing they reference, label or configure on a redacted
//! page survives, a reference bomb under them costs nothing, and what kept
//! pages show or hide in a layer still shows or hides.
mod common;
use common::docs::{get, objects};
use common::leaky::survivors;
use common::probe7::probe7_pdf;
use common::secret::{qdf, rd};
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

#[test]
fn nothing_layer_properties_or_forms_name_on_a_redacted_page_survives() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    // (probe, text that must stay, secrets that must go)
    let cases: [(&str, &[&str], &[&str]); 9] = [
        ("ocpinline", &[], &["ANNOTSECRET", "PAGEDICTSECRET"]),
        ("ocpbomb", &[], &[]),
        (
            "ocplabel",
            &[],
            &["LABELSECRET", "CFGSECRET", "OCGNAMESECRET"],
        ),
        ("ocgusage", &[], &["ANNOTSECRET", "PIECESECRET"]),
        ("ocmd", &[], &[]),
        ("coredfield", &["KEPTVALUE"], &["FIELDSECRET", "CALCSECRET"]),
        ("coind", &["KEPTVALUE"], &["FIELDSECRET", "CALCSECRET"]),
        ("directlink", &[], &["PAGEDICTSECRET"]),
        ("objstm", &[], &["PAGEDICTSECRET", "OBJSTMSECRET"]),
    ];
    let mut bad = Vec::new();
    for (v, kept, gone) in cases {
        let src = probe7_pdf(dir.path(), v);
        let out = match redact1(&src) {
            Ok(out) => out,
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
        if ink(&src) != ink(&out) {
            bad.push(format!("{v}: page 0 shows differently"));
        }
        common::docs::assert_clean(&out);
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn layer_properties_keep_only_kept_groups_and_their_states() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let out = redact1(&probe7_pdf(dir.path(), "ocplabel")).unwrap();
    let o = objects(&out);
    let cat = get(&o, &o["trailer"]["value"]["/Root"]);
    let ocp = get(&o, &cat["/OCProperties"]);
    let g = &get(&o, &ocp["/OCGs"])[0];
    assert_eq!(get(&o, g)["/Name"], "u:L");
    let want = serde_json::json!({ "/OCGs": [g], "/D": { "/Order": [g] } });
    assert_eq!(ocp, &want);
}

#[test]
fn a_layer_reference_bomb_finishes_quickly() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let t = Instant::now();
    let out = redact1(&probe7_pdf(dir.path(), "ocpbomb")).unwrap();
    assert!(t.elapsed() < Duration::from_secs(20), "{:?}", t.elapsed());
    let o = objects(&out);
    let ocp = &get(&o, &o["trailer"]["value"]["/Root"])["/OCProperties"];
    let g = &get(&o, ocp)["/OCGs"][0];
    let want = serde_json::json!({ "/OCGs": [g], "/D": { "/Order": [g] } });
    assert_eq!(get(&o, ocp), &want, "nothing of the bomb is kept");
    common::docs::assert_clean(&out);
}

#[test]
fn a_kept_layer_off_by_base_state_behind_an_ocmd_stays_off() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let src = probe7_pdf(dir.path(), "ocmdoff");
    assert_eq!(ink(&src), 0, "fixture: the layer is off");
    let out = redact1(&src).unwrap();
    assert_eq!(ink(&out), 0, "hidden content revealed");
    common::docs::assert_clean(&out);
}
