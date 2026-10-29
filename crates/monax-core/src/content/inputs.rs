//! Serde input structs of the content commands (see `pages::inputs`).
use super::{BatesOpts, CompressOpts, HfOpts, Metadata, NumberOpts, WatermarkOpts};
use crate::pages::input;
use std::path::PathBuf;

input!(/// Stamps page numbers onto the selected pages: the k-th selected page
    /// (from 0) shows `format` with `%[page]` = `start_at + k`.
    PageNumbersInput { /// Absolute path of the source PDF.
        input: PathBuf, #[serde(default)] opts: NumberOpts }
    "page-numbers", |s, out| &s.input => super::page_numbers(&s.input, &out, s.opts.clone()));
input!(/// Stamps header and footer text (left, center, right) onto the selected
    /// pages; `%[page]` is the 1-based page number, `%[pages]` the page count.
    HeaderFooterInput { /// Absolute path of the source PDF.
        input: PathBuf, #[serde(default)] opts: HfOpts }
    "header-footer", |s, out| &s.input => super::header_footer(&s.input, &out, s.opts.clone()));
input!(/// Stamps watermark text over the content of the selected pages at an
    /// opacity, rotated by an angle; `%[page]` and `%[pages]` expand.
    WatermarkInput { /// Absolute path of the source PDF.
        input: PathBuf, #[serde(default)] opts: WatermarkOpts }
    "watermark", |s, out| &s.input => super::watermark(&s.input, &out, s.opts.clone()));
input!(/// Stamps Bates numbers onto the selected pages: prefix, `start + k`
    /// zero-padded to `digits`, then suffix.
    BatesInput { /// Absolute path of the source PDF.
        input: PathBuf, #[serde(default)] opts: BatesOpts }
    "bates", |s, out| &s.input => super::bates(&s.input, &out, s.opts.clone()));
input!(/// Writes a smaller copy of a PDF: images drawn above `dpi` are
    /// downsampled and JPEG-encoded, streams recompressed; optionally drops
    /// embedded media and metadata. Only 8-bit gray or RGB images that qpdf
    /// can decode are downsampled; CMYK or indexed color, other bit depths,
    /// /Decode arrays, color-key or /Matte masks, JPX/JBIG2/CCITT images and
    /// images drawn only outside page content are left as they are, and the
    /// job's notes count them by reason. Returns the path and those notes.
    CompressInput { /// Absolute path of the source PDF.
        input: PathBuf, #[serde(default)] opts: CompressOpts }
    "compress" -> notes, |s, out| &s.input => super::compress_with_notes(&s.input, &out, s.opts));
input!(/// Sets document information (title, author, subject, keywords,
    /// creator, producer): an omitted entry is kept, an empty string removes it.
    SetMetadataInput { /// Absolute path of the source PDF.
        input: PathBuf, #[serde(default)] metadata: Metadata }
    "metadata", |s, out| &s.input => super::set_metadata(&s.input, &out, s.metadata.clone()));
input!(/// Burns form fields and annotations into the page content as displayed,
    /// so they can no longer be edited; links and hidden annotations are removed.
    FlattenInput { /// Absolute path of the source PDF.
        input: PathBuf }
    "flatten", |s, out| &s.input => super::flatten(&s.input, &out));
input!(/// Rebuilds a damaged PDF (broken cross-reference table, truncated file)
    /// into one that opens; works where other tools report a damaged file.
    RepairInput { /// Absolute path of the damaged PDF.
        input: PathBuf }
    "repair" -> notes, |s, out| &s.input => super::repair_with_notes(&s.input, &out));
input!(/// Writes a linearized (web-optimized) copy whose first page shows before
    /// the whole file has downloaded.
    LinearizeInput { /// Absolute path of the source PDF.
        input: PathBuf }
    "linearize", |s, out| &s.input => super::linearize(&s.input, &out));
input!(/// Removes all non-form annotations (comments, highlights, links,
    /// stamps, media) from the chosen pages; form fields stay with their values.
    DeleteAnnotationsInput { /// Absolute path of the source PDF.
        input: PathBuf, /// 0-based pages to clear; omitted = every page.
        #[serde(default)] pages: Option<Vec<usize>> }
    "no-annotations", |s, out| &s.input => super::delete_annotations(&s.input, &out, s.pages.as_deref()));
