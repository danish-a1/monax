use super::super::build;
use super::super::tests::{geom, r, text};
use crate::annotate::Annot;
use serde_json::{Value, json};

/// `form`'s /BBox through its /Matrix: the user space box it draws in.
fn drawn_at(b: &super::Built) -> Vec<f32> {
    let f = |v: &Value| -> Vec<f32> {
        let a = v.as_array().unwrap();
        a.iter().map(|x| x.as_f64().unwrap() as f32).collect()
    };
    let (bb, m) = (f(&b.form["/BBox"]), f(&b.form["/Matrix"]));
    let at = |x: f32, y: f32| (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]);
    let ((x0, y0), (x1, y1)) = (at(bb[0], bb[1]), at(bb[2], bb[3]));
    vec![x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)]
}

#[test]
fn upright_kinds_lay_out_as_displayed_on_turned_pages() {
    let ft = Annot::FreeText {
        rect: r(20.0, 30.0, 60.0, 150.0),
        text: "Hi".into(),
        font_size: 10.0,
        color: "#000000".into(),
    };
    for (rot, bbox, m) in [
        (90, json!([30, 140, 150, 180]), json!([0, 1, -1, 0, 200, 0])),
        (
            180,
            json!([140, 50, 180, 170]),
            json!([-1, 0, 0, -1, 200, 200]),
        ),
        (270, json!([50, 20, 170, 60]), json!([0, -1, 1, 0, 0, 200])),
    ] {
        let b = build(&ft, &geom(rot));
        assert_eq!((&b.form["/BBox"], &b.form["/Matrix"]), (&bbox, &m), "{rot}");
        assert_eq!(drawn_at(&b), [20.0, 30.0, 60.0, 150.0], "{rot}");
        assert_eq!(b.dict["/Rect"], json!([20, 30, 60, 150]), "{rot}");
        let bb = b.form["/BBox"].as_array().unwrap();
        let first = format!("1 0 0 1 {} ", bb[0].as_f64().unwrap() + 2.0);
        assert!(text(&b).contains(&first), "{rot}: {}", text(&b));
    }
    // Note: the icon square at the displayed top-left is the rect.
    let note = Annot::Note {
        rect: r(20.0, 30.0, 60.0, 150.0),
        text: String::new(),
    };
    let b = build(&note, &geom(90));
    assert_eq!(b.dict["/Rect"], json!([20, 30, 40, 50]));
    assert_eq!(drawn_at(&b), [20.0, 30.0, 40.0, 50.0]);
}
