//! Unit tests of signature placement: placement checks, serde defaults and
//! fitting (no pdfium). Image reading is tested beside `content::picture`.
use super::{Placement, fit_scale};
use crate::pages::Rect;

fn place(rotate: u16, opacity: f32, (left, bottom, right, top): (f32, f32, f32, f32)) -> Placement {
    Placement {
        page: 0,
        rect: Rect {
            left,
            bottom,
            right,
            top,
        },
        rotate,
        opacity,
    }
}

fn msg(p: Placement) -> String {
    p.check().unwrap_err().to_string()
}

#[test]
fn placement_check_names_the_bad_field() {
    let r = (0.0, 0.0, 10.0, 10.0);
    for rot in [0, 90, 180, 270] {
        assert!(place(rot, 1.0, r).check().is_ok());
    }
    assert!(place(0, 0.0, r).check().is_ok());
    assert_eq!(
        msg(place(0, 1.0, (0.0, 0.0, 0.0, 10.0))),
        "invalid input: signature rect is empty"
    );
    assert_eq!(
        msg(place(0, 1.0, (0.0, 5.0, 10.0, 1.0))),
        "invalid input: signature rect is empty"
    );
    assert_eq!(
        msg(place(0, 1.0, (0.0, 0.0, f32::INFINITY, 1.0))),
        "invalid input: signature rect is empty"
    );
    assert!(msg(place(360, 1.0, r)).ends_with("got 360"));
    assert!(msg(place(0, -0.1, r)).contains("opacity must be in 0..=1"));
    assert!(msg(place(0, f32::NAN, r)).contains("opacity must be in 0..=1"));
}

#[test]
fn placement_defaults_to_unturned_and_opaque() {
    let v =
        serde_json::json!({ "page": 2, "rect": { "left": 1, "bottom": 2, "right": 3, "top": 4 } });
    let p: Placement = serde_json::from_value(v).unwrap();
    assert_eq!(
        p,
        Placement {
            page: 2,
            ..place(0, 1.0, (1.0, 2.0, 3.0, 4.0))
        }
    );
    let extra = serde_json::json!({ "page": 0, "rect": { "left": 1, "bottom": 2, "right": 3, "top": 4 }, "x": 1 });
    assert!(serde_json::from_value::<Placement>(extra).is_err());
}

#[test]
fn fit_scale_fits_clamps_and_refuses_empty_ink() {
    let room = (0.0, 0.0, 100.0, 40.0);
    assert_eq!(
        fit_scale((0.0, 0.0, 50.0, 10.0), room, (0.0, f32::INFINITY)).unwrap(),
        2.0
    );
    assert_eq!(
        fit_scale((5.0, 5.0, 55.0, 15.0), room, (0.0, 1.0)).unwrap(),
        1.0
    );
    let small = fit_scale((0.0, 0.0, 1000.0, 10.0), room, (0.5, 1.0));
    assert!(small.unwrap_err().to_string().contains("does not fit"));
    for ink in [
        (3.0, 0.0, 3.0, 10.0),
        (0.0, 4.0, 10.0, 4.0),
        (0.0, f32::NAN, 1.0, 1.0),
    ] {
        let e = fit_scale(ink, room, (0.0, 1.0)).unwrap_err().to_string();
        assert_eq!(e, "invalid input: signature has nothing to draw");
    }
}
