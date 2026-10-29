//! Annotations added to pages: text markup (highlight, underline,
//! strikeout), sticky notes, free text, ink and shapes. Each one is written
//! as a PDF annotation dictionary with its own appearance stream (/AP /N),
//! so it shows the same in every reader; nothing is flattened into the page
//! content. qpdf writes them (`--update-from-json`, as `forms::fill` writes
//! values): pdfium-render 0.9.4 has no creator for /Circle and no access to
//! `FPDFAnnot_SetAP`, /InkList or border widths. pdfium reads each page's
//! MediaBox (inheritance honoured) and measures free text.
//!
//! Coordinates are PDF user space points: origin bottom-left, y-up,
//! relative to the unrotated MediaBox. Pages are 0-based.
mod appearance;
mod check;
mod input;
mod update;

pub use input::AnnotateInput;

use crate::content::qjson::Doc;
use crate::output::{check_out, private_dir, write_output};
use crate::pages::{PagesError, Rect, open, qpdf_op};
use crate::pdfium::{Geom, PdfiumError, count_of, fields, geometry, lib, with_checked};
use pdfium_render::prelude::{
    PdfDocument, PdfPageObjectCommon, PdfPageTextObject, PdfPoints, Pdfium,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A point in PDF user space: points, origin bottom-left, y-up, unrotated
/// MediaBox.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// The outline a shape annotation draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    /// A rectangle along the rect's edges (a /Square annotation).
    Rectangle,
    /// The ellipse inscribed in the rect (a /Circle annotation).
    Ellipse,
}

/// One annotation. Rects and points are PDF user space points (origin
/// bottom-left, y-up, unrotated MediaBox) inside the page's MediaBox.
/// `color` is `#RRGGBB`. A highlight is 40% opaque (multiply blend); every
/// other annotation is opaque.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Annot {
    /// Highlights the rect (text markup; its QuadPoints are the rect).
    Highlight { rect: Rect, color: String },
    /// Underlines the rect (text markup; its QuadPoints are the rect).
    Underline { rect: Rect, color: String },
    /// Strikes through the middle of the rect (text markup; its QuadPoints
    /// are the rect).
    Strikeout { rect: Rect, color: String },
    /// A sticky note holding `text`; its icon sits at the rect's top-left.
    Note { rect: Rect, text: String },
    /// `text` written in the rect: Helvetica at `font_size` points (> 0),
    /// WinAnsi characters only, one line per `\n`; it must fit the rect.
    FreeText {
        rect: Rect,
        text: String,
        font_size: f32,
        color: String,
    },
    /// Freehand lines: 1..=100 strokes of 2..=5000 points each, `width`
    /// 0.25..=20 points.
    Ink {
        strokes: Vec<Vec<Point>>,
        width: f32,
        color: String,
    },
    /// A rectangle or ellipse outline along the rect, `width` 0.25..=20
    /// points.
    Shape {
        kind: ShapeKind,
        rect: Rect,
        color: String,
        width: f32,
    },
}

/// An annotation and the 0-based page it goes on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PageAnnot {
    /// 0-based page index.
    pub page: usize,
    /// The annotation; `type` selects the kind.
    pub annot: Annot,
}

/// Adds `annots` (1..=1000, any pages) to `input`, each with an appearance
/// stream, and writes `out` copy-on-save. Existing annotations, form
/// fields and page content are kept; pages without new annotations are not
/// changed. Returns the path written.
///
/// # Errors
/// `InvalidInput` (bad paths; no annotations or more than 1000; for
/// annotation `i`: a rect or point not finite, empty or outside its page's
/// MediaBox, a bad color, width, font size, stroke or point count, free
/// text with a character outside WinAnsi or that does not fit its rect),
/// `PageOutOfRange`, `EncryptedInput`, `Damaged`, `Pdfium`
/// (`DamagedRepairFirst`: qpdf warns reading it; the form field check),
/// `Qpdf`, `Sidecar`, `Io`.
pub fn save(input: &Path, out: &Path, annots: &[PageAnnot]) -> Result<PathBuf, PagesError> {
    check::count(annots)?;
    open(input)?;
    check_out(out, &[input])?;
    let ok = fields::check(input, None)?;
    let (pages, count) = with_checked(&ok, None, |engine, doc| fit(engine, doc, annots))?;
    let dir = private_dir()?; // document data
    let doc = Doc::read_unrepaired(input, dir.path())?;
    let built = annots
        .iter()
        .map(|a| (a.page, appearance::build(&a.annot, &pages[&a.page])));
    let objects = update::updates(&doc, count, built.collect())?;
    let update = doc.update_arg(objects, dir.path())?;
    write_output(out, &[input], |tmp| {
        qpdf_op(&[input.into(), update, tmp.into()])
    })
}

/// Checks each annotation against its page's MediaBox (pdfium's geometry:
/// inheritance honoured) and that each free text fits its rect as
/// displayed, measured in pdfium's Helvetica. Returns the geometry of each
/// annotated page and the page count.
fn fit(
    engine: &Pdfium,
    doc: &mut PdfDocument<'_>,
    annots: &[PageAnnot],
) -> Result<(HashMap<usize, Geom>, usize), PdfiumError> {
    let mut pages = HashMap::new();
    let font = doc.fonts_mut().helvetica();
    for (i, a) in annots.iter().enumerate() {
        let g = if let Some(&g) = pages.get(&a.page) {
            g
        } else {
            let g = geometry(engine, doc, a.page)?;
            pages.insert(a.page, g);
            g
        };
        check::annot(i, &a.annot, g.media)?;
        if let Annot::FreeText {
            rect,
            text,
            font_size,
            ..
        } = &a.annot
        {
            let size = PdfPoints::new(*font_size);
            let mut widest = 0.0_f32;
            for line in text.lines() {
                let obj = PdfPageTextObject::new(doc, line, font, size).map_err(lib)?;
                widest = widest.max(obj.width().map_err(lib)?.value);
            }
            let shown = appearance::displayed(&g, rect);
            if !appearance::text_fits(&shown, *font_size, text.lines().count(), widest) {
                return Err(PdfiumError::InvalidInput(format!(
                    "annotation {i}: free text does not fit its rect at {font_size} pt"
                )));
            }
        }
    }
    Ok((pages, count_of(doc)))
}
