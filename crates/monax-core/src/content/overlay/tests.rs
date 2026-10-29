use super::*;
use crate::content::Anchor;
use crate::pdfium::Box4;

#[test]
fn place_anchors_inside_margins() {
    let page = (200.0, 100.0);
    let ink = (40.0, 10.0);
    assert_eq!(place(Anchor::BottomLeft, 5.0, page, ink), Some((5.0, 5.0)));
    assert_eq!(
        place(Anchor::BottomCenter, 5.0, page, ink),
        Some((80.0, 5.0))
    );
    assert_eq!(place(Anchor::TopRight, 5.0, page, ink), Some((155.0, 85.0)));
    assert_eq!(place(Anchor::Center, 0.0, page, ink), Some((80.0, 45.0)));
    assert_eq!(place(Anchor::MiddleLeft, 2.0, page, ink), Some((2.0, 45.0)));
    assert_eq!(place(Anchor::BottomRight, 81.0, page, ink), None);
    assert_eq!(place(Anchor::TopCenter, 0.0, page, (201.0, 1.0)), None);
}

#[test]
fn origin_is_the_displayed_bottom_left() {
    let b = (50.0, 20.0, 250.0, 120.0);
    assert_eq!(origin(0, b), (50.0, 20.0));
    assert_eq!(origin(90, b), (250.0, 20.0));
    assert_eq!(origin(180, b), (250.0, 120.0));
    assert_eq!(origin(270, b), (50.0, 120.0));
}

#[test]
fn stamp_check_rejects_bad_fields() {
    let ok = Stamp::plain("x".into(), Anchor::Center, 0.0, 10.0);
    assert!(ok.check().is_ok());
    let msg = |s: Stamp| s.check().unwrap_err().to_string();
    let empty = Stamp {
        text: String::new(),
        ..ok.clone()
    };
    assert_eq!(msg(empty), "invalid input: overlay text is empty");
    assert!(
        msg(Stamp {
            size: 0.0,
            ..ok.clone()
        })
        .contains("size must be")
    );
    assert!(
        msg(Stamp {
            size: f32::NAN,
            ..ok.clone()
        })
        .contains("size must be")
    );
    assert!(
        msg(Stamp {
            margin: -1.0,
            ..ok.clone()
        })
        .contains("margin must be")
    );
    assert!(
        msg(Stamp {
            opacity: 1.5,
            ..ok.clone()
        })
        .contains("opacity must be")
    );
    assert!(
        msg(Stamp {
            angle: f32::INFINITY,
            ..ok.clone()
        })
        .contains("angle must be")
    );
    assert!(
        msg(Stamp {
            text: "\u{3b1}".into(),
            ..ok
        })
        .contains("U+03B1")
    );
}

#[test]
fn line_box_spans_ascent_to_descent_and_rotates() {
    let near = |a: Box4, b: Box4| {
        let d = [a.0 - b.0, a.1 - b.1, a.2 - b.2, a.3 - b.3];
        assert!(d.iter().all(|v| v.abs() < 1e-3), "{a:?} vs {b:?}");
    };
    near(
        line_box((1.0, 41.0), (-2.0, 7.0), 0.0),
        (1.0, -2.0, 41.0, 7.0),
    );
    near(
        line_box((0.0, 10.0), (-2.0, 8.0), 90.0),
        (-8.0, 0.0, 2.0, 10.0),
    );
    near(
        line_box((0.0, 10.0), (-2.0, 8.0), 180.0),
        (-10.0, -8.0, 0.0, 2.0),
    );
}

#[test]
fn shown_inverts_the_display_turn() {
    // CropBox [50 20 250 120]; the redaction pixel tests' rect, y-up.
    let (v, r) = ((50.0, 20.0, 250.0, 120.0), (60.0, 30.0, 100.0, 50.0));
    assert_eq!(shown(0, v, r), (10.0, 10.0, 50.0, 30.0));
    assert_eq!(shown(90, v, r), (10.0, 150.0, 30.0, 190.0));
    assert_eq!(shown(180, v, r), (150.0, 70.0, 190.0, 90.0));
    assert_eq!(shown(270, v, r), (70.0, 10.0, 90.0, 50.0));
}
