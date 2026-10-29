//! Ink and shape appearances: stroked paths in user space.
use super::{base, fmt, num, nums, ops};
use crate::annotate::{Point, ShapeKind};
use crate::pages::Rect;
use serde_json::{Value, json};

/// Bezier control distance of a quarter ellipse, per unit radius.
const KAPPA: f32 = 0.552_284_8;

/// Strokes of `width` in color `c`; the rect holds every point and the
/// line around it.
pub(super) fn ink(strokes: &[Vec<Point>], width: f32, c: [f32; 3]) -> super::Built {
    let all = strokes.iter().flatten();
    let (mut l, mut b, mut r, mut t) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in all {
        (l, b, r, t) = (l.min(p.x), b.min(p.y), r.max(p.x), t.max(p.y));
    }
    let h = width / 2.0;
    let mut draw = format!("{} RG {} w 1 J 1 j", ops(&c), fmt(width));
    let mut list = Vec::new();
    for s in strokes {
        let xy: Vec<f32> = s.iter().flat_map(|p| [p.x, p.y]).collect();
        let path: Vec<String> = xy.chunks(2).map(ops).collect();
        draw.push_str(&format!(
            " {} m {} S",
            path[0],
            path[1..].join(" l ") + " l"
        ));
        list.push(nums(&xy));
    }
    let mut built = base("/Ink", [l - h, b - h, r + h, t + h], c, draw, json!({}));
    built.dict.insert("/InkList".into(), Value::Array(list));
    built.dict.insert("/BS".into(), json!({ "/W": num(width) }));
    built
}

/// A rectangle or ellipse stroked along `r`'s edges; the rect grows by
/// half the line width to hold it.
pub(super) fn shape(kind: ShapeKind, r: &Rect, width: f32, c: [f32; 3]) -> super::Built {
    let h = width / 2.0;
    let (w, ht) = (r.right - r.left, r.top - r.bottom);
    let path = match kind {
        ShapeKind::Rectangle => format!("{} re", ops(&[r.left, r.bottom, w, ht])),
        ShapeKind::Ellipse => ellipse(r),
    };
    let draw = format!("{} RG {} w {path} S", ops(&c), fmt(width));
    let subtype = match kind {
        ShapeKind::Rectangle => "/Square",
        ShapeKind::Ellipse => "/Circle",
    };
    let grown = [r.left - h, r.bottom - h, r.right + h, r.top + h];
    let mut b = base(subtype, grown, c, draw, json!({}));
    b.dict.insert("/BS".into(), json!({ "/W": num(width) }));
    b
}

/// The ellipse inscribed in `r` as four Bezier curves (a closed path).
fn ellipse(r: &Rect) -> String {
    let (cx, cy) = ((r.left + r.right) / 2.0, (r.bottom + r.top) / 2.0);
    let (rx, ry) = ((r.right - r.left) / 2.0, (r.top - r.bottom) / 2.0);
    let (kx, ky) = (KAPPA * rx, KAPPA * ry);
    let quarter = |p: [f32; 6]| format!(" {} c", ops(&p));
    let mut s = format!("{} m", ops(&[cx + rx, cy]));
    s += &quarter([cx + rx, cy + ky, cx + kx, cy + ry, cx, cy + ry]);
    s += &quarter([cx - kx, cy + ry, cx - rx, cy + ky, cx - rx, cy]);
    s += &quarter([cx - rx, cy - ky, cx - kx, cy - ry, cx, cy - ry]);
    s += &quarter([cx + kx, cy - ry, cx + rx, cy - ky, cx + rx, cy]);
    s
}
