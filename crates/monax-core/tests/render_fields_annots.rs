//! R117, R118: pdfium draws a form from qpdf's rewrite of it, so a
//! reference qpdf cannot resolve (pdfium would by number) is gone for
//! both readers and the field DAG behind it is never walked; a widget
//! listed twice is refused (pdfium walks its subtree once per listing), and
//! so is a /Parent chain deeper than pdfium nests fields (it joins the
//! chain into every widget's name).
mod common;
use common::probe17::probe17_pdf;
use monax_core::pdfium::{render, thumbnail_png};
use std::time::{Duration, Instant};

fn render_err(src: &std::path::Path) -> Result<(), String> {
    render(src, 0, 72).map(drop).map_err(|e| e.to_string())
}

/// Each probe at 26 levels: pdfium alone, on the input, walks the DAG for
/// about 11 s (x4 per 2 levels).
#[test]
fn a_reference_qpdf_cannot_resolve_hides_the_dag_from_pdfium_too() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    // An /Annots element, the /Annots list, a field's /Kids.
    for v in ["gen1elem:26", "gen1list:26", "gen1kids:26"] {
        let src = probe17_pdf(dir.path(), v);
        let t = Instant::now();
        assert_eq!(render_err(&src), Ok(()), "{v}");
        thumbnail_png(&src, 0, 64).unwrap();
        assert!(
            t.elapsed() < Duration::from_secs(2),
            "{v}: {:?}",
            t.elapsed()
        );
    }
}

#[test]
fn a_widget_listed_twice_in_annots_is_refused_in_time() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    // 3000 kids listed 3000 times: pdfium alone takes about 20 s (1 s at
    // 1000, 7 s at 2000, 56 s at 4000).
    let src = probe17_pdf(dir.path(), "many:3000");
    let t = Instant::now();
    let twice = "damaged file: form field tree reaches a field twice";
    assert_eq!(render_err(&src), Err(twice.into()));
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
}

#[test]
fn a_parent_chain_deeper_than_pdfium_nests_fields_is_refused_in_time() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    // 5000 widgets under a 5000-field /Parent chain: pdfium alone joins
    // each widget's name in about 30 s (1000 x 10000: 19 s).
    let src = probe17_pdf(dir.path(), "parent:5000");
    let t = Instant::now();
    let deep = "damaged file: form field nesting is deeper than 32";
    assert_eq!(render_err(&src), Err(deep.into()));
    assert!(t.elapsed() < Duration::from_secs(2), "{:?}", t.elapsed());
    // As deep as pdfium nests fields: drawn.
    render(&probe17_pdf(dir.path(), "parent:32"), 0, 72).unwrap();
}

#[test]
fn an_indirect_subtype_is_no_widget_to_pdfium() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    // pdfium loads no field from it: as fast at 26 levels as at 2 (a
    // direct /Widget there takes about 11 s).
    let src = probe17_pdf(dir.path(), "indsub:26");
    let t = Instant::now();
    render(&src, 0, 72).unwrap();
    assert!(t.elapsed() < Duration::from_secs(2), "{:?}", t.elapsed());
}
