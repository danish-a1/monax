//! Pure placement math in displayed and user space (points).
use crate::content::Anchor;
use crate::pdfium::Box4;

/// Bottom-left corner for a `(bw, bh)` box anchored at `at`, `margin` points
/// inside a `(pw, ph)` page; `None` when the box does not fit inside the
/// margins.
pub(super) fn place(
    at: Anchor,
    margin: f32,
    (pw, ph): (f32, f32),
    (bw, bh): (f32, f32),
) -> Option<(f32, f32)> {
    let (fx, fy) = at.fractions();
    let (free_x, free_y) = (pw - 2.0 * margin - bw, ph - 2.0 * margin - bh);
    (free_x >= 0.0 && free_y >= 0.0).then_some((margin + fx * free_x, margin + fy * free_y))
}

/// Axis-aligned box of the text line `x0..x1` (ink extent) by
/// `descent..ascent` (font metrics, baseline at 0) after turning it `angle`
/// degrees counter-clockwise about the origin.
pub(super) fn line_box((x0, x1): (f32, f32), (descent, ascent): (f32, f32), angle: f32) -> Box4 {
    let (sin, cos) = angle.to_radians().sin_cos();
    let corners = [(x0, descent), (x1, descent), (x0, ascent), (x1, ascent)];
    let turned = corners.map(|(x, y)| (x * cos - y * sin, x * sin + y * cos));
    turned.iter().fold(
        (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
        |(l, b, r, t), &(x, y)| (l.min(x), b.min(y), r.max(x), t.max(y)),
    )
}

/// User space point shown at the displayed origin (bottom-left) of a page
/// with visible box `(l, b, r, t)` turned `rot` degrees clockwise. Displayed
/// space maps to user space by turning `rot` counter-clockwise, then moving
/// the origin here.
pub(crate) fn origin(rot: u16, (l, b, r, t): Box4) -> (f32, f32) {
    match rot {
        90 => (r, b),
        180 => (r, t),
        270 => (l, t),
        _ => (l, b),
    }
}

/// User space box `(l, b, r, t)` as displayed on a page with visible box
/// `v` turned `rot` degrees clockwise: points from the displayed
/// bottom-left. The inverse of the map [`origin`] describes.
pub(crate) fn shown(rot: u16, v: Box4, (l, b, r, t): Box4) -> Box4 {
    let (ox, oy) = origin(rot, v);
    let turn = |x: f32, y: f32| {
        let (x, y) = (x - ox, y - oy);
        match rot {
            90 => (y, -x),
            180 => (-x, -y),
            270 => (-y, x),
            _ => (x, y),
        }
    };
    let ((x0, y0), (x1, y1)) = (turn(l, b), turn(r, t));
    (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1))
}
