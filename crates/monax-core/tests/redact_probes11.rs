//! gen11 redaction probes (R108): only what renders kept pages keeps an
//! object page 1 also reaches, so no group (however pdfium finds it)
//! rescues page 1's content; groups and states are read like pdfium, so
//! page 0 shows and hides what it did; a dual-use object leaks nothing.
mod common;
use common::leaky::survivors;
use common::probe11::{GEN11, probe11_pdf};
use common::secret::rd;
use monax_core::pdfium;
use monax_core::security::redact_with_notes;
use std::path::Path;

const LAYERS: &str = "layers used only by redacted pages were removed";
const CHANGED: &str = "redaction check failed: a kept page would change";

/// Dark pixels of page 0 at 72 dpi.
fn ink(p: &Path) -> usize {
    let r = pdfium::render(p, 0, 72).unwrap();
    r.rgba.chunks(4).filter(|px| px[0] < 128).count()
}

#[test]
fn no_group_rescues_a_redacted_page_and_kept_pages_look_the_same() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let mut bad = Vec::new();
    for v in GEN11.split_whitespace() {
        let src = probe11_pdf(dir.path(), v);
        let out = dir.path().join(format!("{v}-out.pdf"));
        let r = redact_with_notes(&src, &out, &[rd(1, 10, 10, 50, 50)]);
        // R115: where the output would draw a kept page differently (a
        // dual-use object cut to a group; UTF-16 strings and dangling
        // values qpdf decodes or drops) it is refused whole.
        let changes = ["dualuntyped", "dualfield", "utf16", "vsnull"].contains(&v);
        let (out, notes) = match (changes, r) {
            (true, Err(e)) if e.to_string() == CHANGED && !out.exists() => continue,
            (false, Ok(r)) => r,
            (_, r) => {
                bad.push(format!("{v}: {r:?}"));
                continue;
            }
        };
        let left = survivors(&out, &["PAGESECRET", "TLSECRET"]);
        let (a, b) = (ink(&src), ink(&out));
        if !left.is_empty() || a != b {
            bad.push(format!("{v}: survived {left:?}, ink {a} -> {b}"));
        }
        if v == "listred" && notes.iter().any(|n| n == LAYERS) {
            bad.push(format!("listred: {notes:?}"));
        }
        common::docs::assert_clean(&out);
    }
    assert!(bad.is_empty(), "{bad:#?}");
}
