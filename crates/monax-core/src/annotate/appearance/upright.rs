//! Appearances upright as the page is displayed (free text, note,
//! underline, strikeout): laid out in displayed space (the visible box
//! turned by /Rotate, origin at its displayed bottom-left, as
//! [`crate::content::shown`]), with a form /Matrix that turns them back to
//! user space.
use super::{Built, ICON_SIDE, base, fmt, nums, ops};
use crate::content::{origin, shown};
use crate::pages::Rect;
use crate::pdfium::Geom;
use serde_json::{Value, json};

/// `r` (user space) as displayed on a page with geometry `g`.
pub(in crate::annotate) fn displayed(g: &Geom, r: &Rect) -> Rect {
    let (left, bottom, right, top) =
        shown(g.rotation, g.visible, (r.left, r.bottom, r.right, r.top));
    Rect {
        left,
        bottom,
        right,
        top,
    }
}

/// Displayed space to user space: turn /Rotate counter-clockwise, then
/// move to the user point at the displayed origin.
fn matrix(g: &Geom) -> [f32; 6] {
    let (ox, oy) = origin(g.rotation, g.visible);
    let (sin, cos) = match g.rotation {
        90 => (1.0, 0.0),
        180 => (0.0, -1.0),
        270 => (-1.0, 0.0),
        _ => (0.0, 1.0),
    };
    [cos, sin, -sin, cos, ox, oy]
}

/// Displayed point `(x, y)` in user space.
fn user(m: &[f32; 6], (x, y): (f32, f32)) -> [f32; 2] {
    [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]]
}

/// QuadPoints of `r` (user space): its displayed upper-left, upper-right,
/// lower-left and lower-right corners, in user space.
pub(super) fn quad(g: &Geom, r: &Rect) -> [f32; 8] {
    let (m, d) = (matrix(g), displayed(g, r));
    let [a, b] = user(&m, (d.left, d.top));
    let [c, e] = user(&m, (d.right, d.top));
    let [f, h] = user(&m, (d.left, d.bottom));
    let [i, j] = user(&m, (d.right, d.bottom));
    [a, b, c, e, f, h, i, j]
}

/// A dictionary of `subtype` whose appearance `draw` is laid out over the
/// displayed box `d`; its /Rect is `d` in user space.
pub(super) fn upright(
    subtype: &str,
    g: &Geom,
    d: &Rect,
    c: [f32; 3],
    draw: Vec<u8>,
    res: Value,
) -> Built {
    let m = matrix(g);
    let ([x0, y0], [x1, y1]) = (user(&m, (d.left, d.bottom)), user(&m, (d.right, d.top)));
    let rect = [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)];
    let mut b = base(subtype, rect, c, String::new(), res);
    b.content = draw;
    b.form["/BBox"] = nums(&[d.left, d.bottom, d.right, d.top]);
    b.form["/Matrix"] = nums(&m);
    b
}

/// An underline (`at` 0.0) or strikeout (`at` 0.5) across `r` as displayed.
pub(super) fn line(subtype: &str, g: &Geom, r: &Rect, c: [f32; 3], at: f32) -> Built {
    let d = displayed(g, r);
    let w = ((d.top - d.bottom) / 14.0).max(0.5);
    let y = d.bottom + w + (d.top - d.bottom - 2.0 * w) * at;
    let draw = format!(
        "{} RG {} w {} m {} l S",
        ops(&c),
        fmt(w),
        ops(&[d.left, y]),
        ops(&[d.right, y])
    );
    let mut b = upright(subtype, g, &d, c, draw.into_bytes(), json!({}));
    b.dict.insert("/QuadPoints".into(), nums(&quad(g, r)));
    b
}

/// A yellow note icon at the displayed top-left of `r`; the icon square is
/// the annotation's rect.
pub(super) fn note(g: &Geom, r: &Rect, text: &str) -> Built {
    let c = [1.0, 0.82, 0.0];
    let d = displayed(g, r);
    let s = ICON_SIDE.min(d.right - d.left).min(d.top - d.bottom);
    let (x, y) = (d.left, d.top - s);
    let mut draw = format!(
        "{} rg 0 0 0 RG 0.5 w {} re B",
        ops(&c),
        ops(&[x + 0.25, y + 0.25, s - 0.5, s - 0.5])
    );
    for k in [0.3, 0.5, 0.7] {
        let at = |f: f32| ops(&[x + f * s, y + k * s]);
        draw.push_str(&format!(" {} m {} l S", at(0.2), at(0.8)));
    }
    let icon = Rect {
        left: x,
        bottom: y,
        right: x + s,
        top: d.top,
    };
    let mut b = upright("/Text", g, &icon, c, draw.into_bytes(), json!({}));
    b.dict
        .insert("/Contents".into(), json!(format!("u:{text}")));
    b.dict.insert("/Name".into(), json!("/Comment"));
    b.dict.insert("/Open".into(), json!(false));
    b
}

#[cfg(test)]
mod tests;
