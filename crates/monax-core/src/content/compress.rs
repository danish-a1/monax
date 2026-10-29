//! compress: downsample images to a dpi, re-encode as JPEG, recompress
//! streams, optionally drop media and metadata.
use super::PagesError;
use super::annots::remove_annots;
use super::images::downsample;
use super::qjson::Doc;
use super::rewrite::qpdf_write;
use crate::output::{private_dir, private_file};
use crate::pages::open;
use crate::pdfium::{Extents, count_of, fields, image_extents, lib, save, with_checked, with_doc};
use pdfium_render::prelude::PdfPageAnnotationType as Kind;
use pdfium_render::prelude::{PdfDocument, Pdfium};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Options of [`compress`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct CompressOpts {
    /// Images drawn at more pixels per inch than this (as placed on the page)
    /// are downsampled to it; 1..=2400.
    pub dpi: u32,
    /// JPEG quality of re-encoded images, 1 (smallest) ..= 100 (best).
    pub jpeg_quality: u8,
    /// Remove embedded files and media annotations (file attachment, sound,
    /// movie, screen, rich media, 3D).
    pub discard_media: bool,
    /// Remove the document information dictionary and XMP metadata.
    pub strip_metadata: bool,
}

impl Default for CompressOpts {
    fn default() -> Self {
        Self {
            dpi: 150,
            jpeg_quality: 75,
            discard_media: false,
            strip_metadata: false,
        }
    }
}

fn is_media(k: Kind) -> bool {
    use Kind::*;
    matches!(
        k,
        FileAttachment | Sound | Movie | Screen | RichMedia | ThreeD
    )
}

/// Drawn image sizes of `input`; with `discard`, also drops media
/// annotations and embedded files into a copy in `dir`. Returns the sizes
/// and the file to continue from.
fn scan(input: &Path, dir: &Path, discard: bool) -> Result<(Extents, PathBuf), PagesError> {
    // Walking the widgets builds pdfium's form: check its fields first.
    let ok = discard.then(|| fields::check(input, None)).transpose()?;
    let run = |_: &Pdfium, doc: &mut PdfDocument<'_>| {
        let ext = image_extents(doc)?;
        let Some(ok) = &ok else {
            return Ok((ext, None));
        };
        for i in 0..count_of(doc) {
            remove_annots(doc, i, is_media, ok)?;
        }
        let files = doc.attachments_mut();
        for i in (0..files.len()).rev() {
            files.delete_at_index(i).map_err(lib)?;
        }
        Ok((ext, Some(save(doc)?)))
    };
    let (ext, bytes) = match &ok {
        Some(c) => with_checked(c, None, run),
        None => with_doc(input, run),
    }?;
    let Some(bytes) = bytes else {
        return Ok((ext, input.to_path_buf()));
    };
    let work = dir.join("work.pdf");
    private_file(&work)?;
    std::fs::write(&work, bytes)?;
    Ok((ext, work))
}

/// Writes a smaller copy of `input` to `out`: images drawn above `dpi`
/// pixels per inch are downsampled to it and JPEG-encoded at
/// `jpeg_quality` (see `images` for which images qualify), other lossless
/// 8-bit images are JPEG-encoded at that quality when it makes them smaller
/// (qpdf `--optimize-images`), streams are recompressed at the highest flate
/// level and objects packed into object streams. Page content streams and
/// geometry are unchanged. Returns the path written.
///
/// # Errors
/// `InvalidInput` (bad paths, `dpi` outside 1..=2400, `jpeg_quality`
/// outside 1..=100), `EncryptedInput`, `Damaged`, `Qpdf`, `Sidecar`,
/// `Pdfium` (among them, when discarding media, `FieldRevisit`: the form's
/// field tree reaches a field twice), `Io`.
pub fn compress(input: &Path, out: &Path, opts: CompressOpts) -> Result<PathBuf, PagesError> {
    compress_with_notes(input, out, opts).map(|(p, _)| p)
}

/// [`compress`], also returning one note per reason images were left above
/// `dpi`, with a count ("2 images left above 72 dpi: color space not gray
/// or RGB"); empty when every image drawn above `dpi` was resampled.
///
/// # Errors
/// As [`compress`].
pub fn compress_with_notes(
    input: &Path,
    out: &Path,
    opts: CompressOpts,
) -> Result<(PathBuf, Vec<String>), PagesError> {
    let CompressOpts {
        dpi,
        jpeg_quality: q,
        ..
    } = opts;
    if !(1..=2400).contains(&dpi) {
        return Err(PagesError::InvalidInput(format!(
            "dpi must be in 1..=2400, got {dpi}"
        )));
    }
    if !(1..=100).contains(&q) {
        return Err(PagesError::InvalidInput(format!(
            "JPEG quality must be in 1..=100, got {q}"
        )));
    }
    open(input)?;
    let dir = private_dir()?; // document data
    let (ext, work) = scan(input, dir.path(), opts.discard_media)?;
    let doc = Doc::read(&work, dir.path())?;
    let (updates, notes) = downsample(&doc, &work, &ext, dpi, q, dir.path())?;
    let mut args: Vec<OsString> = Vec::new();
    if !updates.is_empty() {
        args.push(doc.update_arg(updates, dir.path())?);
    }
    let quality = format!("--jpeg-quality={q}");
    let flags = [
        "--optimize-images",
        &quality,
        "--recompress-flate",
        "--compression-level=9",
        "--object-streams=generate",
    ];
    args.extend(flags.map(OsString::from));
    if opts.strip_metadata {
        args.extend(["--remove-metadata".into(), "--remove-info".into()]);
    }
    Ok((qpdf_write(input, &work, args, out)?, notes))
}
