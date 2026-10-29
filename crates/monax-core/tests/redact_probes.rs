//! Redaction probes: references that reach a redacted page's annotations,
//! form widgets or content from elsewhere leave nothing behind, and the
//! leak check does not stop valid redactions.
mod common;
use common::leaky::{assert_absent, survivors};
use common::probe::probe_pdf;
use common::secret::{qdf, rd};
use monax_core::security::redact_with_notes;

const LAYERS: &str = "layers used only by redacted pages were removed";

type Run = Result<(std::path::PathBuf, Vec<String>), String>;

fn try_run(v: &str, dir: &tempfile::TempDir) -> Run {
    let src = probe_pdf(dir.path(), v);
    let r = redact_with_notes(&src, &dir.path().join("o.pdf"), &[rd(1, 10, 10, 50, 50)]);
    r.map_err(|e| format!("{v}: {e}"))
}

fn run(v: &str) -> (std::path::PathBuf, Vec<String>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let (out, notes) = try_run(v, &dir).unwrap();
    (out, notes, dir)
}

#[test]
fn nothing_reachable_from_elsewhere_survives() {
    require_qpdf!();
    let cases: [(&str, &[&str]); 6] = [
        ("hide", &["NOTESECRET"]),
        ("catannot", &["NOTESECRET"]),
        ("catcontent", &["PAGESECRET"]),
        ("twowidget", &["CAPSECRET", "APSECRET"]),
        ("radio", &["RADIOSECRET"]),
        ("nested", &["NESTSECRET"]),
    ];
    let mut bad = Vec::new();
    let dir = tempfile::tempdir().unwrap();
    for (v, secrets) in cases {
        let out = match try_run(v, &dir) {
            Ok((out, _)) => out,
            Err(e) => {
                bad.push(e);
                continue;
            }
        };
        let left = survivors(&out, &[secrets, &["PAGESECRET"]].concat());
        if !left.is_empty() {
            bad.push(format!("{v}: {left:?}"));
        }
        common::docs::assert_clean(&out);
    }
    assert!(bad.is_empty(), "survived: {bad:?}");
    let (out, _, _d) = run("twowidget");
    assert!(
        qdf(&out).contains("(shown)"),
        "field keeps its kept widget and value"
    );
}

#[test]
fn layers_only_redacted_pages_use_are_dropped_and_noted() {
    require_qpdf!();
    let (out, notes, _d) = run("ocg");
    assert_absent(&out, &["OCGSECRET"]);
    common::docs::assert_clean(&out);
    assert!(notes.iter().any(|n| n == LAYERS), "{notes:?}");
}

#[test]
fn the_leak_check_does_not_stop_valid_redactions() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let r: Vec<Run> = ["tounicode", "kern"].map(|v| try_run(v, &dir)).into();
    let failed: Vec<&String> = r.iter().filter_map(|r| r.as_ref().err()).collect();
    assert!(failed.is_empty(), "{failed:?}");
    for (out, _) in r.into_iter().flatten() {
        assert_absent(&out, &["Made with", "the port"]);
        common::docs::assert_clean(&out);
    }
}
