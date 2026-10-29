//! The serde input of [`super::save`] (GUI command and MCP tool).
use super::PageAnnot;
use crate::pages::input;
use std::path::PathBuf;

input!(/// Adds annotations to a PDF and writes a new PDF: highlight,
    /// underline, strikeout, sticky note, free text, ink and shapes
    /// (rectangle, ellipse), on any pages, each with its own appearance so
    /// it shows in every reader. Rects and points are PDF user space points
    /// (origin bottom-left, y-up, unrotated MediaBox) inside the page's
    /// MediaBox; colors are `#RRGGBB`. Existing annotations, form fields
    /// and page content are kept.
    AnnotateInput { /// Absolute path of the source PDF.
        input: PathBuf, /// 1..=1000 annotations, each with its 0-based page.
        annots: Vec<PageAnnot> }
    "annotated", |s, out| &s.input => super::save(&s.input, &out, &s.annots));
