use super::*;

const MEDIA: Box4 = (0.0, 0.0, 200.0, 100.0);

fn r(left: f32, bottom: f32, right: f32, top: f32) -> Rect {
    Rect {
        left,
        bottom,
        right,
        top,
    }
}

fn why(i: usize, a: Annot) -> String {
    match annot(i, &a, MEDIA) {
        Err(PdfiumError::InvalidInput(s)) => s,
        other => panic!("expected InvalidInput, got {other:?}"),
    }
}

fn hl(rect: Rect, color: &str) -> Annot {
    Annot::Highlight {
        rect,
        color: color.into(),
    }
}

fn ink(strokes: Vec<Vec<Point>>, width: f32) -> Annot {
    Annot::Ink {
        strokes,
        width,
        color: "#000000".into(),
    }
}

fn pts(n: usize, x: f32) -> Vec<Point> {
    vec![Point { x, y: 10.0 }; n]
}

fn on(annot: Annot) -> super::super::PageAnnot {
    super::super::PageAnnot { page: 0, annot }
}

#[test]
fn counts_one_to_a_thousand() {
    let hl = || on(hl(r(1.0, 1.0, 9.0, 9.0), "#000000"));
    assert!(count(&[hl()]).is_ok() && count(&vec![hl(); MAX]).is_ok());
    let msg = |v: &[super::super::PageAnnot]| count(v).unwrap_err().to_string();
    assert_eq!(msg(&[]), "invalid input: no annotations given");
    assert_eq!(
        msg(&vec![hl(); 1001]),
        "invalid input: too many annotations: 1001 (at most 1000)"
    );
}

#[test]
fn ink_points_are_capped_across_the_request() {
    let two = |n| on(ink(vec![pts(n, 1.0), pts(n, 2.0)], 1.0));
    assert!(count(&[two(50_000), two(50_000)]).is_ok());
    assert_eq!(
        count(&[two(50_000), two(50_000), two(1)])
            .unwrap_err()
            .to_string(),
        "invalid input: too many ink points: 200002 in all (at most 200000)"
    );
}

#[test]
fn colors_are_hash_rrggbb() {
    assert_eq!(color("#FF8000"), Some([1.0, 128.0 / 255.0, 0.0]));
    assert_eq!(color("#00ff00"), Some([0.0, 1.0, 0.0]));
    for s in [
        "FF8000",
        "#FF800",
        "#FF80000",
        "#GG0000",
        "red",
        "",
        "#\u{e9}\u{e9}\u{e9}",
    ] {
        assert_eq!(color(s), None, "{s}");
    }
    assert_eq!(
        why(2, hl(r(1.0, 1.0, 9.0, 9.0), "red")),
        "annotation 2: color must be #RRGGBB, got \"red\""
    );
}

#[test]
fn rects_are_finite_non_empty_and_inside_the_media_box() {
    assert!(annot(0, &hl(r(0.0, 0.0, 200.0, 100.0), "#000000"), MEDIA).is_ok());
    let empty = "annotation 0: rect is empty or not finite";
    assert_eq!(why(0, hl(r(5.0, 5.0, 5.0, 9.0), "#000000")), empty);
    assert_eq!(why(0, hl(r(5.0, 9.0, 8.0, 5.0), "#000000")), empty);
    assert_eq!(why(0, hl(r(f32::NAN, 5.0, 8.0, 9.0), "#000000")), empty);
    let out = "annotation 3: rect is outside the page's MediaBox";
    assert_eq!(why(3, hl(r(150.0, 5.0, 201.0, 9.0), "#000000")), out);
    assert_eq!(why(3, hl(r(-1.0, 5.0, 8.0, 9.0), "#000000")), out);
    let note = Annot::Note {
        rect: r(1.0, 90.0, 9.0, 101.0),
        text: "x".into(),
    };
    assert_eq!(why(3, note), out);
}

#[test]
fn ink_counts_points_and_width() {
    assert!(annot(0, &ink(vec![pts(2, 1.0); 100], 0.25), MEDIA).is_ok());
    assert!(annot(0, &ink(vec![pts(5000, 1.0)], 20.0), MEDIA).is_ok());
    let w = |a| why(1, a);
    assert_eq!(
        w(ink(vec![], 1.0)),
        "annotation 1: ink needs 1..=100 strokes, got 0"
    );
    assert_eq!(
        w(ink(vec![pts(2, 1.0); 101], 1.0)),
        "annotation 1: ink needs 1..=100 strokes, got 101"
    );
    assert_eq!(
        w(ink(vec![pts(2, 1.0), pts(1, 1.0)], 1.0)),
        "annotation 1: ink stroke 1 needs 2..=5000 points, got 1"
    );
    assert_eq!(
        w(ink(vec![pts(5001, 1.0)], 1.0)),
        "annotation 1: ink stroke 0 needs 2..=5000 points, got 5001"
    );
    assert_eq!(
        w(ink(vec![pts(2, 201.0)], 1.0)),
        "annotation 1: ink point is outside the page's MediaBox"
    );
    assert_eq!(
        w(ink(vec![pts(2, f32::INFINITY)], 1.0)),
        "annotation 1: ink point is not finite"
    );
    assert_eq!(
        w(ink(vec![pts(2, 1.0)], 0.2)),
        "annotation 1: width must be 0.25..=20 points, got 0.2"
    );
    assert_eq!(
        w(ink(vec![pts(2, 1.0)], f32::NAN)),
        "annotation 1: width must be 0.25..=20 points, got NaN"
    );
}

#[test]
fn shapes_check_rect_width_and_color() {
    let s = |width, color: &str| Annot::Shape {
        kind: super::super::ShapeKind::Ellipse,
        rect: r(1.0, 1.0, 9.0, 9.0),
        color: color.into(),
        width,
    };
    assert!(annot(0, &s(2.0, "#123456"), MEDIA).is_ok());
    assert_eq!(
        why(0, s(21.0, "#123456")),
        "annotation 0: width must be 0.25..=20 points, got 21"
    );
    assert_eq!(
        why(0, s(2.0, "#12345")),
        "annotation 0: color must be #RRGGBB, got \"#12345\""
    );
}

#[test]
fn free_text_needs_text_a_size_and_winansi() {
    let ft = |text: &str, font_size| Annot::FreeText {
        rect: r(1.0, 1.0, 99.0, 50.0),
        text: text.into(),
        font_size,
        color: "#000000".into(),
    };
    assert!(annot(0, &ft("caf\u{e9}\nline two\r\nthree", 12.0), MEDIA).is_ok());
    assert_eq!(why(0, ft(" \n ", 12.0)), "annotation 0: free text is empty");
    assert_eq!(
        why(0, ft("ok", 0.0)),
        "annotation 0: font size must be a positive number of points, got 0"
    );
    assert_eq!(
        why(0, ft("a\n\u{3b1}", 12.0)),
        "annotation 0: character '\u{3b1}' (U+03B1) is not supported by Helvetica (WinAnsi)"
    );
}
