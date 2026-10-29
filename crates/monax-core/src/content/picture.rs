//! Images placed on pages (signatures, images to PDF): PNG or JPEG by magic
//! bytes, checked against the raster pixel budget. A PNG is decoded to 8-bit
//! BGRA (pdfium keeps its alpha as a soft mask); a JPEG is handed to pdfium
//! as is (DCTDecode). Errors name the image as the caller calls it (`what`).
mod jfif;

use crate::check_input_file;
use crate::pdfium::{MAX_PIXELS, PdfiumError, lib};
use jfif::jpeg_size;
use pdfium_render::prelude::{PdfBitmap, PdfBitmapFormat, PdfDocument, PdfPageImageObject};
use std::io::{Cursor, Read};
use std::path::Path;

const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";
const JPEG_MAGIC: &[u8] = &[0xFF, 0xD8, 0xFF];
/// Largest image file read, bytes (64 MiB).
const MAX_FILE: u64 = 64 * 1024 * 1024;

/// An image read from disk, ready for pdfium.
pub(crate) enum Picture {
    Jpeg {
        bytes: Vec<u8>,
        /// How errors name it.
        what: String,
    },
    /// `w` x `h` pixels, 4 bytes each (BGRA, or BGRx when `alpha` is false).
    Png {
        w: i32,
        h: i32,
        px: Vec<u8>,
        alpha: bool,
    },
}

fn bad(m: String) -> PdfiumError {
    PdfiumError::InvalidInput(m)
}

/// Fails when `w` x `h` pixels of image `what` are over the raster budget.
fn budget(what: &str, w: u64, h: u64) -> Result<(), PdfiumError> {
    if w * h <= MAX_PIXELS {
        return Ok(());
    }
    Err(bad(format!(
        "{what} is {w}x{h} px, over the {MAX_PIXELS} pixel budget"
    )))
}

fn not_jpeg(what: &str) -> PdfiumError {
    bad(format!("{what} is not a valid JPEG"))
}

/// The cheap checks of [`read`], from the path and the first 8 bytes:
/// absolute, an existing regular file, at most 64 MiB, PNG or JPEG magic.
///
/// # Errors
/// `InvalidInput` naming `what`, `Io`.
pub(crate) fn sniff(path: &Path, what: &str) -> Result<(), PdfiumError> {
    check_input_file(path)?;
    let len = std::fs::metadata(path)?.len();
    if len > MAX_FILE {
        let m = format!("{what} is {len} bytes, over the {MAX_FILE} byte limit");
        return Err(bad(m));
    }
    let mut head = Vec::with_capacity(8);
    std::fs::File::open(path)?.take(8).read_to_end(&mut head)?;
    if head.starts_with(JPEG_MAGIC) || head.starts_with(PNG_MAGIC) {
        return Ok(());
    }
    Err(bad(format!("{what} is not a PNG or JPEG")))
}

/// The bytes of `path`, never more than `cap` + 1 read: the file may have
/// grown since [`sniff`] checked its size.
///
/// # Errors
/// `InvalidInput` (over `cap` bytes), `Io`.
fn read_capped(path: &Path, cap: u64, what: &str) -> Result<Vec<u8>, PdfiumError> {
    let mut buf = Vec::new();
    std::fs::File::open(path)?
        .take(cap + 1)
        .read_to_end(&mut buf)?;
    if buf.len() as u64 > cap {
        return Err(bad(format!("{what} is over the {cap} byte limit")));
    }
    Ok(buf)
}

/// Reads the image at `path` (absolute, an existing regular file, at most
/// 64 MiB); errors call it `what` ("signature image"). Each is checked
/// against the pixel budget from its header: a JPEG must have quantization
/// tables, a frame header and scan data (R134), a PNG is then decoded.
///
/// # Errors
/// `InvalidInput` (bad path, file over 64 MiB, not a PNG or JPEG, over
/// budget, an invalid PNG or JPEG), `Io`.
pub(crate) fn read(path: &Path, what: &str) -> Result<Picture, PdfiumError> {
    sniff(path, what)?;
    let bytes = read_capped(path, MAX_FILE, what)?;
    if bytes.starts_with(JPEG_MAGIC) {
        let (w, h) = jpeg_size(&bytes).ok_or_else(|| not_jpeg(what))?;
        budget(what, w.into(), h.into())?;
        let what = what.into();
        return Ok(Picture::Jpeg { bytes, what });
    }
    let invalid = |e: String| bad(format!("{what} is not a valid PNG: {e}"));
    // IHDR comes first: width and height at bytes 16..24, big-endian.
    let dim = |at: usize| {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    };
    let (w, h) = dim(16)
        .zip(dim(20))
        .ok_or_else(|| invalid("truncated".into()))?;
    budget(what, w.into(), h.into())?;
    let mut d = png::Decoder::new(Cursor::new(&bytes));
    d.set_transformations(png::Transformations::normalize_to_color8());
    let mut r = d.read_info().map_err(|e| invalid(e.to_string()))?;
    let mut buf = vec![
        0;
        r.output_buffer_size()
            .ok_or_else(|| invalid("too large".into()))?
    ];
    let info = r.next_frame(&mut buf).map_err(|e| invalid(e.to_string()))?;
    buf.truncate(info.buffer_size());
    let n = info.color_type.samples();
    let alpha = n == 2 || n == 4;
    let px = match n {
        4 => {
            buf.as_chunks_mut::<4>()
                .0
                .iter_mut()
                .for_each(|p| p.swap(0, 2));
            buf
        }
        3 => buf
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[2], p[1], p[0], 255])
            .collect(),
        _ => buf
            .chunks_exact(n)
            .flat_map(|p| [p[0], p[0], p[0], if alpha { p[1] } else { 255 }])
            .collect(),
    };
    let (w, h) = (info.width as i32, info.height as i32);
    Ok(Picture::Png { w, h, px, alpha })
}

impl Picture {
    /// EXIF Orientation of a JPEG (1..=8, 1 when absent or unreadable);
    /// 1 for a PNG.
    pub(crate) fn orientation(&self) -> u16 {
        match self {
            Picture::Jpeg { bytes, .. } => jfif::orientation(bytes),
            Picture::Png { .. } => 1,
        }
    }

    /// A new image object in `doc`, one point per pixel (the aspect ratio),
    /// and its size in pixels.
    ///
    /// # Errors
    /// `InvalidInput` (a JPEG pdfium cannot read or reads as empty),
    /// `Library`.
    pub(crate) fn object<'a>(
        &mut self,
        doc: &PdfDocument<'a>,
    ) -> Result<(PdfPageImageObject<'a>, (i32, i32)), PdfiumError> {
        let (mut img, w, h) = match self {
            Picture::Jpeg { bytes, what } => {
                let img = PdfPageImageObject::new_from_jpeg_reader(doc, Cursor::new(&bytes[..]))
                    .map_err(|_| not_jpeg(what))?;
                let (w, h) = (img.width().map_err(lib)?, img.height().map_err(lib)?);
                if w <= 0 || h <= 0 {
                    return Err(not_jpeg(what));
                }
                (img, w, h)
            }
            Picture::Png { w, h, px, alpha } => {
                let f = if *alpha {
                    PdfBitmapFormat::BGRA
                } else {
                    PdfBitmapFormat::BGRx
                };
                let bmp = PdfBitmap::from_bytes(*w, *h, f, px).map_err(lib)?;
                let mut img = PdfPageImageObject::new(doc).map_err(lib)?;
                img.set_bitmap(&bmp).map_err(lib)?;
                (img, *w, *h)
            }
        };
        img.scale(w as f32, h as f32).map_err(lib)?;
        Ok((img, (w, h)))
    }
}

#[cfg(test)]
mod tests;
