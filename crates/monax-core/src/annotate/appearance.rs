//! Each annotation as a PDF annotation dictionary plus the form XObject of
//! its normal appearance (/AP /N), drawn in user space: the form's /BBox is
//! the annotation's /Rect and its matrix is the identity. Values are qpdf
//! JSON (names `/X`, text strings `u:`).
use super::Annot;
use super::check::color;
use crate::pages::Rect;
use crate::pdfium::Geom;
use serde_json::{Map, Value, json};

mod shapes;
mod text;
mod upright;

pub(super) use text::text_fits;
pub(super) use upright::displayed;

/// Highlight opacity (multiply blend); everything else is opaque.
const HIGHLIGHT_ALPHA: f32 = 0.4;
/// Side of a note's icon, points (smaller when the rect is).
const ICON_SIDE: f32 = 20.0;

/// An annotation ready to write.
pub(super) struct Built {
    /// The annotation dictionary, without /AP and /P.
    pub dict: Map<String, Value>,
    /// The appearance form's dictionary.
    pub form: Value,
    /// The appearance form's content stream.
    pub content: Vec<u8>,
}

/// `v` rounded to 3 decimals: shortest decimal text, no exponent.
fn fmt(v: f32) -> String {
    let r = (f64::from(v) * 1000.0).round() / 1000.0;
    format!("{}", r + 0.0) // + 0.0: no "-0"
}

/// `v` rounded to 3 decimals as a JSON number (an integer when whole).
pub(super) fn num(v: f32) -> Value {
    let r = (f64::from(v) * 1000.0).round() / 1000.0;
    match r.fract() == 0.0 && r.abs() < 1e15 {
        true => json!(r as i64),
        false => json!(r),
    }
}

fn nums(v: &[f32]) -> Value {
    Value::Array(v.iter().map(|&x| num(x)).collect())
}

fn ops(v: &[f32]) -> String {
    v.iter().map(|&x| fmt(x)).collect::<Vec<_>>().join(" ")
}

/// The annotation `a` (already checked) on a page with geometry `g` as a
/// dictionary and appearance.
pub(super) fn build(a: &Annot, g: &Geom) -> Built {
    use Annot::*;
    let rgb = |c: &str| color(c).unwrap_or_default();
    match a {
        Highlight { rect, color } => {
            let c = rgb(color);
            let gs = json!({ "/GS0": { "/Type": "/ExtGState", "/CA": num(HIGHLIGHT_ALPHA),
                "/ca": num(HIGHLIGHT_ALPHA), "/BM": "/Multiply" } });
            let w = [
                rect.left,
                rect.bottom,
                rect.right - rect.left,
                rect.top - rect.bottom,
            ];
            let draw = format!("/GS0 gs {} rg {} re f", ops(&c), ops(&w));
            let res = json!({ "/ExtGState": gs });
            let mut b = base("/Highlight", corners(rect), c, draw, res);
            b.dict
                .insert("/QuadPoints".into(), nums(&upright::quad(g, rect)));
            b.dict.insert("/CA".into(), num(HIGHLIGHT_ALPHA));
            b
        }
        Underline { rect, color } => upright::line("/Underline", g, rect, rgb(color), 0.0),
        Strikeout { rect, color } => upright::line("/StrikeOut", g, rect, rgb(color), 0.5),
        Note { rect, text } => upright::note(g, rect, text),
        FreeText {
            rect,
            text,
            font_size,
            color,
        } => text::free_text(g, rect, text, *font_size, rgb(color)),
        Ink {
            strokes,
            width,
            color,
        } => shapes::ink(strokes, *width, rgb(color)),
        Shape {
            kind,
            rect,
            color,
            width,
        } => shapes::shape(*kind, rect, *width, rgb(color)),
    }
}

/// A dictionary of `subtype` at `rect` (printable, color `c`) whose
/// appearance draws `draw` with `resources`.
fn base(subtype: &str, rect: [f32; 4], c: [f32; 3], draw: String, resources: Value) -> Built {
    let mut dict = Map::new();
    dict.insert("/Type".into(), json!("/Annot"));
    dict.insert("/Subtype".into(), json!(subtype));
    dict.insert("/Rect".into(), nums(&rect));
    dict.insert("/C".into(), nums(&c));
    dict.insert("/F".into(), json!(4));
    let form = json!({ "/Type": "/XObject", "/Subtype": "/Form", "/BBox": nums(&rect),
        "/Resources": resources });
    Built {
        dict,
        form,
        content: draw.into_bytes(),
    }
}

fn corners(r: &Rect) -> [f32; 4] {
    [r.left, r.bottom, r.right, r.top]
}

#[cfg(test)]
mod tests;
