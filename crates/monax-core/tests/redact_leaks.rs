//! Nothing of a redacted page survives, whatever shape its resources,
//! tags and annotations take; document-level text goes and is noted.
mod common;
use common::leaky::{GONE, KEPT, SHAPES, assert_absent, leaky_pdf};
use common::secret::{qdf, rd, secret_pdf};
use monax_core::pdfium;
use monax_core::security::redact_with_notes;

const SEARCH: &str = "text on 1 redacted page(s) is no longer searchable";
const TAGS: &str = "accessibility tags were removed";
const DESTS: &str = "links to redacted page(s) no longer have a target";
const DROPPED: &str = "document-level features were removed: ";
const SHARED: &str = "form field values and objects shared with kept pages are not redacted";

#[test]
fn nothing_of_a_redacted_page_survives_in_any_shape() {
    require_qpdf!();
    for shape in SHAPES {
        let dir = tempfile::tempdir().unwrap();
        let src = leaky_pdf(dir.path(), shape);
        let all = qdf(&src);
        assert!(GONE.iter().all(|s| all.contains(s)), "{shape:?}: fixture");
        let (out, notes) =
            redact_with_notes(&src, &dir.path().join("o.pdf"), &[rd(1, 10, 10, 50, 50)]).unwrap();
        assert_absent(&out, &[&GONE[..], &KEPT[..]].concat());
        assert!(qdf(&out).contains("(keep)"), "{shape:?}: field on page 0");
        let dropped = format!("{DROPPED}bookmarks, named destinations, the open action");
        assert_eq!(notes, [SEARCH, TAGS, DESTS, &dropped, SHARED], "{shape:?}");
        assert_eq!(
            pdfium::extract_text(&out, 0..=0).unwrap(),
            "Hello",
            "{shape:?}"
        );
        common::docs::assert_clean(&out);
    }
}

#[test]
fn untagged_documents_without_links_get_only_their_notes() {
    require_qpdf!();
    let (dir, three) = common::setup();
    let (_, notes) =
        redact_with_notes(&three, &dir.path().join("o.pdf"), &[rd(0, 1, 1, 9, 9)]).unwrap();
    assert_eq!(notes, [SEARCH, SHARED]);
    let src = secret_pdf(dir.path());
    let (_, notes) =
        redact_with_notes(&src, &dir.path().join("s.pdf"), &[rd(1, 1, 1, 9, 9)]).unwrap();
    let dropped = format!("{DROPPED}bookmarks");
    assert_eq!(notes, [SEARCH, &dropped, SHARED], "bookmark to page 1");
}
