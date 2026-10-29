//! annotate on a /Rotate 90 page with a cropped origin: free text, notes and
//! text markup are upright as displayed. Also: a page tree that lists a
//! page twice, and the cap on ink points per request.
mod common;
use common::annot::*;
use common::forms::derive;
use common::sign::near;
use common::{copy_fixture, diff_box, names, setup};
use monax_core::annotate::{Annot, save};
use monax_core::pages::PagesError;
use serde_json::json;

// rotated-crop.pdf: MediaBox [0 0 300 200], CropBox [50 20 250 120],
// /Rotate 90. User rect (150 25 180 115) shows at displayed x 5..95,
// y 70..100 from the bottom: pixels x 5..95, y 100..130 from the top.
const R: (f32, f32, f32, f32) = (150.0, 25.0, 180.0, 115.0);

fn one(a: Annot) -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let (dir, rot) = copy_fixture("rotated-crop.pdf", "rot.pdf");
    let out = save(&rot, &dir.path().join("o.pdf"), &[on(0, a)]).unwrap();
    (dir, rot, out)
}

#[test]
fn free_text_reads_left_to_right_as_displayed() {
    require_qpdf!();
    let (_d, rot, out) = one(free_text(rect(R.0, R.1, R.2, R.3), "Hello World", 10.0));
    let (x0, y0, x1, y1) = diff_box(&rot, &out, 0).unwrap();
    assert!(
        x1 - x0 > 3 * (y1 - y0),
        "not a horizontal line: {:?}",
        (x0, y0, x1, y1)
    );
    assert!(
        x0 >= 5 && x1 <= 95 && y0 >= 100 && y1 <= 130,
        "{:?}",
        (x0, y0, x1, y1)
    );
    assert!(
        x0 <= 9 && y0 <= 105,
        "not at the displayed top-left: {:?}",
        (x0, y0)
    );
}

#[test]
fn free_text_must_fit_the_rect_as_displayed() {
    require_qpdf!();
    // 30 pt wide in user x but 90 pt wide as displayed: fits.
    let (dir, rot) = copy_fixture("rotated-crop.pdf", "rot.pdf");
    let wide = free_text(rect(R.0, R.1, R.2, R.3), "Hello World Wide", 10.0);
    assert!(save(&rot, &dir.path().join("o.pdf"), &[on(0, wide)]).is_ok());
    // Two lines need 20.75 pt + margins; only 30 - 4 is there as displayed.
    let tall = free_text(rect(R.0, R.1, R.2, R.3), "A\nB\nC", 10.0);
    let r = save(&rot, &dir.path().join("o.pdf"), &[on(0, tall)]);
    let m = "invalid input: annotation 0: free text does not fit its rect at 10 pt";
    assert_eq!(r.unwrap_err().to_string(), m);
}

#[test]
fn note_icon_sits_at_the_displayed_top_left_and_is_its_rect() {
    require_qpdf!();
    let note = Annot::Note {
        rect: rect(R.0, R.1, R.2, R.3),
        text: "n".into(),
    };
    let (_d, rot, out) = one(note);
    near(diff_box(&rot, &out, 0).unwrap(), (5, 100, 24, 119));
    // Displayed (5 80 25 100) is user x 150..170, y 25..45.
    assert_eq!(
        nums(&dicts(&out)[0][0], "/Rect"),
        [150.0, 25.0, 170.0, 45.0]
    );
}

#[test]
fn underline_runs_along_the_displayed_bottom() {
    require_qpdf!();
    let u = Annot::Underline {
        rect: rect(R.0, R.1, R.2, R.3),
        color: "#0000FF".into(),
    };
    let (_d, rot, out) = one(u);
    let (x0, y0, x1, y1) = diff_box(&rot, &out, 0).unwrap();
    assert!(x1 - x0 > 80 && y1 - y0 < 6, "{:?}", (x0, y0, x1, y1));
    assert!(
        (124..=130).contains(&y1),
        "not at the displayed bottom: {y1}"
    );
    // QuadPoints: displayed upper-left, upper-right, lower-left, lower-right.
    let q = nums(&dicts(&out)[0][0], "/QuadPoints");
    assert_eq!(q, [150.0, 25.0, 150.0, 115.0, 180.0, 25.0, 180.0, 115.0]);
}

#[test]
fn a_page_listed_twice_in_the_page_tree_is_refused() {
    require_qpdf!();
    let (dir, three) = setup();
    let kids = json!({ "obj:2 0 R": { "value": {
        "/Type": "/Pages", "/Kids": ["3 0 R", "4 0 R", "3 0 R"], "/Count": 3 } } });
    let twice = derive(&three, "twice.pdf", kids);
    let a = [on(2, highlight(rect(10.0, 10.0, 50.0, 30.0)))];
    let r = save(&twice, &dir.path().join("o.pdf"), &a);
    assert_eq!(r.unwrap_err().to_string(), "damaged file: repair it first");
    assert_eq!(names(dir.path()), ["three.pdf", "twice.pdf"]);
}

#[test]
fn ink_points_are_capped_per_request() {
    require_qpdf!();
    let (dir, three) = setup();
    let stroke: Vec<(f32, f32)> = (0..5000).map(|i| (10.0, 10.0 + (i % 100) as f32)).collect();
    let strokes: Vec<&[(f32, f32)]> = vec![&stroke; 41];
    let r = save(
        &three,
        &dir.path().join("o.pdf"),
        &[on(0, ink(&strokes, 1.0))],
    );
    let m = "invalid input: too many ink points: 205000 in all (at most 200000)";
    assert!(matches!(&r, Err(PagesError::InvalidInput(_))), "{r:?}");
    assert_eq!(r.unwrap_err().to_string(), m);
    assert_eq!(names(dir.path()), ["three.pdf"]);
}
