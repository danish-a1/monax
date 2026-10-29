use super::*;
use crate::annotate::{Point, ShapeKind};
use crate::pages::Rect;
use crate::pdfium::Geom;

pub(super) fn r(left: f32, bottom: f32, right: f32, top: f32) -> Rect {
    Rect {
        left,
        bottom,
        right,
        top,
    }
}

/// A 200 x 200 pt page turned `rotation` degrees, visible box from (0, 0).
pub(super) fn geom(rotation: u16) -> Geom {
    let page = (0.0, 0.0, 200.0, 200.0);
    Geom {
        media: page,
        visible: page,
        rotation,
    }
}

fn b0(a: Annot) -> Built {
    build(&a, &geom(0))
}

pub(super) fn text(b: &Built) -> String {
    String::from_utf8_lossy(&b.content).into_owned()
}

#[test]
fn numbers_round_to_three_decimals() {
    assert_eq!(fmt(60.0), "60");
    assert_eq!(fmt(215.0 / 255.0), "0.843");
    assert_eq!(fmt(-0.0001), "0");
    assert_eq!(fmt(-1.25), "-1.25");
    assert_eq!(num(20.0), json!(20));
    assert_eq!(num(0.4), json!(0.4));
}

#[test]
fn highlight_fills_its_rect_at_forty_percent_multiplied() {
    let b = b0(Annot::Highlight {
        rect: r(40.0, 60.0, 140.0, 100.0),
        color: "#FFD700".into(),
    });
    assert_eq!(text(&b), "/GS0 gs 1 0.843 0 rg 40 60 100 40 re f");
    assert_eq!(b.dict["/Subtype"], "/Highlight");
    assert_eq!(b.dict["/CA"], json!(0.4));
    assert_eq!(
        b.dict["/QuadPoints"],
        json!([40, 100, 140, 100, 40, 60, 140, 60])
    );
    assert_eq!(b.form["/BBox"], json!([40, 60, 140, 100]));
    let gs = &b.form["/Resources"]["/ExtGState"]["/GS0"];
    assert_eq!(
        (&gs["/BM"], &gs["/CA"], &gs["/ca"]),
        (&json!("/Multiply"), &json!(0.4), &json!(0.4))
    );
}

#[test]
fn underline_and_strikeout_stroke_across_the_rect() {
    let rect = r(10.0, 10.0, 50.0, 24.0);
    let color = || "#0000FF".to_string();
    let u = b0(Annot::Underline {
        rect,
        color: color(),
    });
    assert_eq!(text(&u), "0 0 1 RG 1 w 10 11 m 50 11 l S");
    let s = b0(Annot::Strikeout {
        rect,
        color: color(),
    });
    assert_eq!(text(&s), "0 0 1 RG 1 w 10 17 m 50 17 l S");
    assert_eq!(s.dict["/Subtype"], "/StrikeOut");
    assert!(s.dict.get("/CA").is_none());
}

#[test]
fn note_icon_sits_at_the_top_left() {
    let b = b0(Annot::Note {
        rect: r(10.0, 10.0, 50.0, 50.0),
        text: "hi".into(),
    });
    assert!(
        text(&b)
            .starts_with("1 0.82 0 rg 0 0 0 RG 0.5 w 10.25 30.25 19.5 19.5 re B 14 36 m 26 36 l S")
    );
    assert_eq!(b.dict["/Rect"], json!([10, 30, 30, 50]));
    assert_eq!(
        (&b.dict["/Contents"], &b.dict["/Name"]),
        (&json!("u:hi"), &json!("/Comment"))
    );
    let small = b0(Annot::Note {
        rect: r(10.0, 10.0, 18.0, 50.0),
        text: String::new(),
    });
    assert!(
        text(&small).contains(" 10.25 42.25 7.5 7.5 re B"),
        "{}",
        text(&small)
    );
}

#[test]
fn free_text_lines_are_helvetica_winansi_from_the_top_left() {
    let b = b0(Annot::FreeText {
        rect: r(10.0, 10.0, 190.0, 100.0),
        text: "caf\u{e9} (a)\\\nnext".into(),
        font_size: 10.0,
        color: "#FF0000".into(),
    });
    let mut want = b"BT /Helv 10 Tf 1 0 0 rg 1 0 0 1 12 90.82 Tm (caf\xe9 \\(a\\)\\\\) Tj".to_vec();
    want.extend(b" 1 0 0 1 12 79.32 Tm (next) Tj ET");
    assert_eq!(b.content, want);
    assert_eq!(b.dict["/DA"], "u:/Helv 10 Tf 1 0 0 rg");
    assert!(b.dict.get("/C").is_none());
    let font = &b.form["/Resources"]["/Font"]["/Helv"];
    assert_eq!(
        (&font["/BaseFont"], &font["/Encoding"]),
        (&json!("/Helvetica"), &json!("/WinAnsiEncoding"))
    );
}

#[test]
fn free_text_fits_inside_the_margin() {
    let rect = r(0.0, 0.0, 104.0, 15.25);
    assert!(text_fits(&rect, 10.0, 1, 100.0));
    assert!(!text_fits(&rect, 10.0, 1, 100.5));
    assert!(!text_fits(&rect, 10.0, 2, 10.0));
    assert!(text_fits(&r(0.0, 0.0, 104.0, 26.75), 10.0, 2, 10.0));
}

#[test]
fn ink_strokes_each_list_and_grow_the_rect() {
    let p = |x, y| Point { x, y };
    let b = b0(Annot::Ink {
        strokes: vec![
            vec![p(20.0, 20.0), p(60.0, 60.0)],
            vec![p(70.0, 30.0), p(90.0, 30.0), p(90.0, 40.0)],
        ],
        width: 3.0,
        color: "#FF0000".into(),
    });
    assert_eq!(
        text(&b),
        "1 0 0 RG 3 w 1 J 1 j 20 20 m 60 60 l S 70 30 m 90 30 l 90 40 l S"
    );
    assert_eq!(
        b.dict["/InkList"],
        json!([[20, 20, 60, 60], [70, 30, 90, 30, 90, 40]])
    );
    assert_eq!(b.dict["/Rect"], json!([18.5, 18.5, 91.5, 61.5]));
    assert_eq!(b.dict["/BS"], json!({ "/W": 3 }));
}

#[test]
fn shapes_stroke_the_rect_edges() {
    let s = |kind| {
        b0(Annot::Shape {
            kind,
            rect: r(110.0, 20.0, 180.0, 90.0),
            color: "#0000FF".into(),
            width: 2.0,
        })
    };
    let rect = s(ShapeKind::Rectangle);
    assert_eq!(text(&rect), "0 0 1 RG 2 w 110 20 70 70 re S");
    assert_eq!(
        (&rect.dict["/Subtype"], &rect.dict["/Rect"]),
        (&json!("/Square"), &json!([109, 19, 181, 91]))
    );
    let e = s(ShapeKind::Ellipse);
    assert_eq!(e.dict["/Subtype"], "/Circle");
    let c = text(&e);
    assert!(
        c.starts_with("0 0 1 RG 2 w 180 55 m 180 74.33 164.33 90 145 90 c"),
        "{c}"
    );
    assert!(c.ends_with(" 180 55 c S"), "{c}");
    assert_eq!(c.matches(" c").count(), 4);
}
