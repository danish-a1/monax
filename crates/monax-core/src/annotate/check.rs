//! Annotation checks: the count, and each annotation's fields against the
//! MediaBox of its page. Messages name the annotation by its 0-based index.
use super::{Annot, PageAnnot, Point};
use crate::content::check_winansi;
use crate::pages::Rect;
use crate::pdfium::{Box4, PdfiumError};

/// Most annotations one call adds.
pub(super) const MAX: usize = 1000;
const STROKES: std::ops::RangeInclusive<usize> = 1..=100;
const POINTS: std::ops::RangeInclusive<usize> = 2..=5000;
/// Most ink points one call adds, all strokes of all ink annotations.
pub(super) const MAX_POINTS: usize = 200_000;
const WIDTH: std::ops::RangeInclusive<f32> = 0.25..=20.0;

fn bad(s: String) -> PdfiumError {
    PdfiumError::InvalidInput(s)
}

/// Fails unless `annots` holds 1..=[`MAX`] annotations.
pub(super) fn count(annots: &[PageAnnot]) -> Result<(), PdfiumError> {
    match annots.len() {
        0 => Err(bad("no annotations given".into())),
        n if n > MAX => Err(bad(format!("too many annotations: {n} (at most {MAX})"))),
        _ => Ok(()),
    }?;
    let ink = |a: &PageAnnot| match &a.annot {
        Annot::Ink { strokes, .. } => strokes.iter().map(Vec::len).sum(),
        _ => 0,
    };
    match annots.iter().map(ink).sum::<usize>() {
        n if n > MAX_POINTS => Err(bad(format!(
            "too many ink points: {n} in all (at most {MAX_POINTS})"
        ))),
        _ => Ok(()),
    }
}

/// `#RRGGBB` as RGB components 0.0..=1.0; `None` for anything else.
pub(super) fn color(s: &str) -> Option<[f32; 3]> {
    let hex = s.strip_prefix('#').filter(|h| h.len() == 6)?;
    let c = |i: usize| {
        hex.get(i..i + 2)
            .and_then(|h| u8::from_str_radix(h, 16).ok())
    };
    Some([c(0)?, c(2)?, c(4)?].map(|v| f32::from(v) / 255.0))
}

/// Checks annotation `i` against `media`, the MediaBox of its page.
///
/// # Errors
/// `InvalidInput` starting `annotation <i>: ` and the reason.
pub(super) fn annot(i: usize, a: &Annot, media: Box4) -> Result<(), PdfiumError> {
    fields(a, media).map_err(|why| bad(format!("annotation {i}: {why}")))
}

fn fields(a: &Annot, media: Box4) -> Result<(), String> {
    use Annot::*;
    match a {
        Highlight { rect, color } | Underline { rect, color } | Strikeout { rect, color } => {
            within(rect, media)?;
            hex(color)
        }
        Note { rect, .. } => within(rect, media),
        FreeText {
            rect,
            text,
            font_size,
            color,
        } => {
            within(rect, media)?;
            hex(color)?;
            crate::content::check_size(*font_size, "font size").map_err(reason)?;
            if text.trim().is_empty() {
                return Err("free text is empty".into());
            }
            text.lines().try_for_each(check_winansi)
        }
        Ink {
            strokes,
            width,
            color,
        } => {
            if !STROKES.contains(&strokes.len()) {
                return Err(format!("ink needs 1..=100 strokes, got {}", strokes.len()));
            }
            for (k, s) in strokes.iter().enumerate() {
                if !POINTS.contains(&s.len()) {
                    return Err(format!(
                        "ink stroke {k} needs 2..=5000 points, got {}",
                        s.len()
                    ));
                }
                s.iter().try_for_each(|p| point(p, media))?;
            }
            stroke(*width)?;
            hex(color)
        }
        Shape {
            rect, color, width, ..
        } => {
            within(rect, media)?;
            stroke(*width)?;
            hex(color)
        }
    }
}

/// The message of an `InvalidInput` from a shared check.
fn reason(e: PdfiumError) -> String {
    match e {
        PdfiumError::InvalidInput(s) => s,
        e => e.to_string(),
    }
}

fn hex(s: &str) -> Result<(), String> {
    match color(s) {
        Some(_) => Ok(()),
        None => Err(format!("color must be #RRGGBB, got {s:?}")),
    }
}

fn stroke(w: f32) -> Result<(), String> {
    match WIDTH.contains(&w) {
        true => Ok(()),
        false => Err(format!("width must be 0.25..=20 points, got {w}")),
    }
}

fn inside((x, y): (f32, f32), (l, b, r, t): Box4) -> bool {
    (l..=r).contains(&x) && (b..=t).contains(&y)
}

fn within(r: &Rect, media: Box4) -> Result<(), String> {
    let v = [r.left, r.bottom, r.right, r.top];
    if !v.iter().all(|v| v.is_finite()) || r.left >= r.right || r.bottom >= r.top {
        return Err("rect is empty or not finite".into());
    }
    match inside((r.left, r.bottom), media) && inside((r.right, r.top), media) {
        true => Ok(()),
        false => Err("rect is outside the page's MediaBox".into()),
    }
}

fn point(p: &Point, media: Box4) -> Result<(), String> {
    if !(p.x.is_finite() && p.y.is_finite()) {
        return Err("ink point is not finite".into());
    }
    match inside((p.x, p.y), media) {
        true => Ok(()),
        false => Err("ink point is outside the page's MediaBox".into()),
    }
}

#[cfg(test)]
mod tests;
