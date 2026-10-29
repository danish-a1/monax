//! PDF pages to image files, one page at a time: each is rendered as the
//! user sees it, encoded and published before the next is drawn, and the
//! pdfium lock is free between pages.
use super::ImageFormat;
use crate::ProgressSink;
use crate::content::encode_jpeg;
use crate::output::{check_out_dir, remove_all, stem};
use crate::pages::{PagesError, check_pages, write_bytes};
use crate::pdfium::{
    Checked, PdfiumError, count_of, encode_png, fields, page_of, px_size, render_page, with_checked,
};
use std::path::{Path, PathBuf};

/// Renders pages of `input` as displayed (/Rotate and CropBox applied,
/// annotations and form fields drawn, white background) at `dpi` (72..=600;
/// `ceil(points * dpi / 72)` pixels a side, at most 64 Mpx a page) to
/// `<stem>-<n>.png` or `.jpg` (`n` the 1-based page number) in `out_dir`.
/// `pages` are 0-based, in output order (`None`: every page);
/// `jpeg_quality` (1..=100) applies to JPEG. A taken name gets `-1`, `-2`,
/// ... `progress` gets a share per page; cancelling it stops before the
/// next page. On any failure or cancel every file written is removed.
/// Returns the files written, in order.
///
/// # Errors
/// `InvalidInput` (bad paths, `dpi` or `jpeg_quality` out of range, empty
/// `pages`, a page over the pixel budget or JPEG's 65535 px side at this
/// dpi), `PageOutOfRange`, `Cancelled`, `EncryptedInput`, `Damaged`,
/// `Pdfium` (the form field check of [`crate::pdfium::render`]), `Io`.
pub fn pdf_to_images(
    input: &Path,
    out_dir: &Path,
    dpi: u32,
    format: ImageFormat,
    jpeg_quality: u8,
    pages: Option<&[usize]>,
    progress: &ProgressSink,
) -> Result<Vec<PathBuf>, PagesError> {
    let bad = |m: String| Err(PagesError::InvalidInput(m));
    if !(72..=600).contains(&dpi) {
        return bad(format!("dpi {dpi} not in 72..=600"));
    }
    if !(1..=100).contains(&jpeg_quality) {
        return bad(format!("jpeg_quality {jpeg_quality} not in 1..=100"));
    }
    check_out_dir(out_dir)?;
    let ok = fields::check(input, None)?;
    let count = with_checked(&ok, None, |_, doc| Ok(count_of(doc)))?;
    if count == 0 {
        return bad("document has no pages".into());
    }
    let pages = match pages {
        Some(p) => check_pages(p, count).map(|()| p.to_vec())?,
        None => (0..count).collect(),
    };
    let (stem, ext) = (stem(input), ext(format));
    // Removes every file published so far on an error, a cancel or a panic.
    let mut written = Written(Vec::with_capacity(pages.len()));
    for (k, &i) in pages.iter().enumerate() {
        if progress.is_cancelled() {
            return Err(PagesError::Cancelled);
        }
        let bytes = image(&ok, i, dpi, format, jpeg_quality)?;
        let name = out_dir.join(format!("{stem}-{}.{ext}", i + 1));
        written
            .0
            .push(write_bytes::<PagesError>(&name, input, &bytes)?);
        progress.set((k + 1) as f32 / pages.len() as f32);
    }
    Ok(std::mem::take(&mut written.0))
}

fn ext(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
    }
}

/// Encoded image of 0-based page `i` of the checked document `ok`.
fn image(
    ok: &Checked,
    i: usize,
    dpi: u32,
    format: ImageFormat,
    quality: u8,
) -> Result<Vec<u8>, PdfiumError> {
    // Messages name the page 1-based, as the file `<stem>-<n>` does.
    let n = i + 1;
    let over =
        |m: &str| PdfiumError::InvalidInput(format!("page {n}: {m} at {dpi} dpi; lower the dpi"));
    let r = with_checked(ok, None, |_, doc| {
        let p = page_of(doc, i)?;
        // Its only error: the pixel budget.
        let (w, h) = px_size(p.width().value, p.height().value, dpi).map_err(|e| match e {
            PdfiumError::InvalidInput(m) => over(&m),
            e => e,
        })?;
        if format == ImageFormat::Jpeg && w.max(h) > JPEG_SIDE {
            return Err(over(&format!(
                "{w}x{h} px is over JPEG's {JPEG_SIDE} px side"
            )));
        }
        render_page(&p, w, h, ok)
    })?;
    match format {
        ImageFormat::Png => encode_png(r.w, r.h, &r.rgba),
        ImageFormat::Jpeg => {
            let size = (r.w, r.h);
            encode_jpeg(&r.rgb(), size, 3, quality)
                .ok_or_else(|| PdfiumError::Library(format!("JPEG encoding of page {n} failed")))
        }
    }
}

/// Longest JPEG side, pixels.
const JPEG_SIDE: i32 = 65535;

/// Files a conversion has published; removed when dropped, so taken out
/// (emptied) on success.
struct Written(Vec<PathBuf>);

impl Drop for Written {
    fn drop(&mut self) {
        remove_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::Written;

    #[test]
    fn written_files_are_removed_on_unwind() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("a.png");
        std::fs::write(&f, b"x").unwrap();
        let g = f.clone();
        let r = std::panic::catch_unwind(move || {
            let _w = Written(vec![g]);
            panic!("mid-loop");
        });
        assert!(r.is_err());
        assert!(!f.exists());
    }
}
