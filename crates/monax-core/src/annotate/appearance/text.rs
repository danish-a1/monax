//! Free text appearance: Helvetica (WinAnsi) lines from the rect's
//! top-left, inside a margin.
use super::upright::{displayed, upright};
use super::{Built, fmt, num, ops};
use crate::content::winansi_byte;
use crate::pages::Rect;
use crate::pdfium::Geom;
use serde_json::json;

/// Inner margin, points.
const PAD: f32 = 2.0;
/// Helvetica's ascender, descender and line spacing per point of size.
const ASCENT: f32 = 0.718;
const DESCENT: f32 = 0.207;
const LEADING: f32 = 1.15;

/// Whether `lines` lines of free text at `size` points, the widest
/// `widest` points wide, fit inside `r` (as displayed) with the margin.
pub(in crate::annotate) fn text_fits(r: &Rect, size: f32, lines: usize, widest: f32) -> bool {
    let tall = (ASCENT + DESCENT + LEADING * lines.saturating_sub(1) as f32) * size;
    widest + 2.0 * PAD <= r.right - r.left && tall + 2.0 * PAD <= r.top - r.bottom
}

/// `line` as a PDF literal string in WinAnsiEncoding (checked text).
fn literal(line: &str) -> Vec<u8> {
    let mut out = vec![b'('];
    for c in line.chars() {
        if matches!(c, '(' | ')' | '\\') {
            out.push(b'\\');
        }
        out.extend(winansi_byte(c));
    }
    out.push(b')');
    out
}

/// `text`, one line per `\n`, at `size` points in color `c`, upright from
/// the displayed top-left of `r`.
pub(super) fn free_text(g: &Geom, r: &Rect, text: &str, size: f32, c: [f32; 3]) -> Built {
    let r = &displayed(g, r);
    let x = r.left + PAD;
    let mut draw = format!("BT /Helv {} Tf {} rg", fmt(size), ops(&c)).into_bytes();
    for (k, line) in text.lines().enumerate() {
        let y = r.top - PAD - (ASCENT + LEADING * k as f32) * size;
        draw.extend(format!(" 1 0 0 1 {} Tm ", ops(&[x, y])).bytes());
        draw.extend(literal(line));
        draw.extend(b" Tj");
    }
    draw.extend(b" ET");
    let font = json!({ "/Type": "/Font", "/Subtype": "/Type1", "/BaseFont": "/Helvetica",
        "/Encoding": "/WinAnsiEncoding" });
    let res = json!({ "/Font": { "/Helv": font } });
    let mut b = upright("/FreeText", g, r, c, draw, res);
    // /C of a free text is its background in some readers: none here.
    b.dict.remove("/C");
    let da = format!("u:/Helv {} Tf {} rg", fmt(size), ops(&c));
    b.dict.insert("/DA".into(), json!(da));
    b.dict
        .insert("/Contents".into(), json!(format!("u:{text}")));
    b.dict.insert("/BS".into(), json!({ "/W": num(0.0) }));
    b
}
