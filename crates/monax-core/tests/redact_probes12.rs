//! gen12 / gen13 / gen14 redaction probes (R108, R110 - R113): hostile
//! syntax around layers never crashes, hangs or reveals; input qpdf warns
//! on or cannot read whole is refused; long chains stay fast and a field
//! DAG is refused before pdfium walks it; plain
//! data never keeps page 1's private dictionaries; every resource kept
//! pages draw with stays.
mod common;
use common::leaky::survivors;
use common::probe12::{GEN12, probe12_pdf};
use common::probe13::probe13_pdf;
use common::probe14::probe14_pdf;
use common::secret::rd;
use monax_core::pdfium;
use monax_core::security::redact_with_notes;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Dark pixels of page 0 at 72 dpi.
fn ink(p: &Path) -> usize {
    let r = pdfium::render(p, 0, 72).unwrap();
    r.rgba.chunks(4).filter(|px| px[0] < 128).count()
}

const REPAIR: &str = "damaged file: repair it first, then redact";

/// The exact refusal each refused probe gets, nothing written: qpdf warns
/// reading it (R112: nested past qpdf's limit, a parse error, a broken
/// xref, no `endobj`), its JSON nests past the reader's, or its field tree
/// revisits a field (R113).
fn refusal(v: &str) -> Option<&'static str> {
    match v {
        "deepcat" | "deepocg" | "unterm" | "badxref" | "noendobj" | "many" | "deepocgoff_1000" => {
            Some(REPAIR)
        }
        "deepocgoff_200" => Some("damaged file: object nesting too deep to redact safely"),
        "fielddag_22" => Some("damaged file: form field tree reaches a field twice"),
        _ => None,
    }
}

/// One `check` at a time: they share the process-wide pdfium lock, so a
/// timed refusal would otherwise count the seconds it queued behind a
/// sibling test's renders (2 - 6.6 s measured against a 25 ms refusal).
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// Redacts page 1 of each of `srcs` (each within `limit`); what went wrong.
fn check(srcs: Vec<PathBuf>, limit: Duration) -> Vec<String> {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let mut bad = Vec::new();
    for src in srcs {
        let v = src.file_stem().unwrap().to_string_lossy().into_owned();
        let out = src.with_file_name(format!("{v}-out.pdf"));
        let t = Instant::now();
        let r = redact_with_notes(&src, &out, &[rd(1, 10, 10, 50, 50)]);
        if t.elapsed() > limit {
            bad.push(format!("{v}: took {:?}", t.elapsed()));
        }
        let out = match (refusal(&v), r) {
            (Some(want), Err(e)) if e.to_string() == want && !out.exists() => continue,
            (Some(_), r) => {
                bad.push(format!("{v}: not refused as it should be: {r:?}"));
                continue;
            }
            (None, Ok((out, _))) => out,
            (None, Err(e)) => {
                bad.push(format!("{v}: {e}"));
                continue;
            }
        };
        let left = survivors(&out, &["PAGESECRET", "PIECESECRET"]);
        let (a, b) = (ink(&src), ink(&out));
        if !left.is_empty() || a != b {
            bad.push(format!("{v}: survived {left:?}, ink {a} -> {b}"));
        }
        common::docs::assert_clean(&out);
    }
    bad
}

#[test]
fn hostile_layer_syntax_and_plain_data_neither_crash_hang_reveal_nor_leak() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let srcs = GEN12
        .split_whitespace()
        .map(|v| probe12_pdf(dir.path(), v))
        .collect();
    let bad = check(srcs, Duration::from_secs(20));
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn every_drawing_resource_of_a_kept_page_stays() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let srcs = ["drawall", "diff"]
        .map(|v| probe13_pdf(dir.path(), v))
        .into();
    let bad = check(srcs, Duration::from_secs(20));
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn over_deep_input_is_refused_long_chains_are_fast_and_plain_dictionaries_draw() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    // 1000: past qpdf's nesting limit; 200: past the JSON reader's.
    // Alone, with the pdfium queue out of the clock: refusals 25 - 35 ms,
    // plain dictionaries 0.2 s, so 5 s is 25x+ headroom. The 100000-link
    // chain is a real redaction (qpdf plus pdfium over 100k objects, 0.8 s)
    // and gets the 20 s the sibling long-input probes (redact_probes7) use.
    let probe = |vs: &str| -> Vec<PathBuf> {
        vs.split_whitespace()
            .map(|v| probe14_pdf(dir.path(), v))
            .collect()
    };
    let mut bad = check(
        probe("deepocgoff:1000 deepocgoff:200 fielddag:22 bs cidsys cidreg"),
        Duration::from_secs(5),
    );
    bad.extend(check(probe("ocgchain:100000"), Duration::from_secs(20)));
    assert!(bad.is_empty(), "{bad:#?}");
}
