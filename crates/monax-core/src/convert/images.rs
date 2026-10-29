//! Images to PDF: each image becomes a one-page PDF in a private temp dir
//! (one image decoded at a time), and qpdf joins the pages into the output.
use super::input::no_images;
use super::{PageSize, SIDE};
use crate::ProgressSink;
use crate::content::picture::{self, Picture};
use crate::output::{check_out, private_dir, private_file};
use crate::pages::{PagesError, join};
use crate::pdfium::{PdfiumError, lib, save, with_engine};
use pdfium_render::prelude::{PdfPageObjectsCommon, PdfPagePaperSize, PdfPoints};
use std::path::{Path, PathBuf};

/// Most images one conversion takes.
const MAX_IMAGES: usize = 500;
/// Writes one PDF with a page per image of `inputs` (1..=500 absolute
/// paths of PNG or JPEG files, told apart by their bytes), in order.
/// Without `page_size` a page is its image's size at 1 pixel = 1 point
/// (at most 14400 points a side); with one, every page has that size and
/// its image is fitted and centred, aspect kept. A JPEG shows as its EXIF
/// Orientation says (size and fit use the oriented size) and is embedded
/// as is; PNG transparency is kept. Every path and file type is checked
/// before any image is decoded. `progress` gets a share per image;
/// cancelling it stops before the next image. Returns the path written
/// (copy-on-save); a failed or cancelled run publishes nothing.
///
/// # Errors
/// `InvalidInput` (no images or over 500, a bad path, an image that is not
/// a valid PNG or JPEG, over 64 MiB or over the 64 Mpx budget (each naming
/// the file), a page size out of range, a bad `out`), `Sidecar` (qpdf
/// missing), `Cancelled`, `Qpdf`, `Pdfium`, `Io`.
pub fn images_to_pdf(
    inputs: &[PathBuf],
    out: &Path,
    page_size: Option<PageSize>,
    progress: &ProgressSink,
) -> Result<PathBuf, PagesError> {
    if inputs.is_empty() {
        return Err(no_images());
    }
    if inputs.len() > MAX_IMAGES {
        let n = inputs.len();
        let m = format!("{n} images given, at most {MAX_IMAGES}");
        return Err(PagesError::InvalidInput(m));
    }
    page_size.as_ref().map(PageSize::check).transpose()?;
    let what = |p: &Path| format!("image {}", p.display());
    // Fail fast: every path and file type before any image is decoded.
    for image in inputs {
        picture::sniff(image, &what(image))?;
    }
    let refs: Vec<&Path> = inputs.iter().map(PathBuf::as_path).collect();
    check_out(out, &refs)?; // before the costly pages
    let dir = private_dir()?; // image data: private, removed on every path
    let mut parts = Vec::with_capacity(inputs.len());
    for (i, image) in inputs.iter().enumerate() {
        if progress.is_cancelled() {
            return Err(PagesError::Cancelled);
        }
        let what = what(image);
        let pic = picture::read(image, &what)?;
        let part = dir.path().join(format!("{i}.pdf"));
        private_file(&part)?;
        std::fs::write(&part, page(pic, page_size, &what)?)?;
        parts.push(part);
        progress.set((i + 1) as f32 / (inputs.len() + 1) as f32);
    }
    join(&parts, out, &refs)
}

/// Bytes of a one-page PDF showing `pic` (see [`images_to_pdf`]).
fn page(mut pic: Picture, size: Option<PageSize>, what: &str) -> Result<Vec<u8>, PdfiumError> {
    with_engine(|engine| {
        let mut doc = engine.create_new_pdf().map_err(lib)?;
        let o = pic.orientation();
        let (mut img, (w, h)) = pic.object(&doc)?;
        let (w, h) = (w as f32, h as f32);
        let [a, b, c, d, e, f] = matrix(o, w, h);
        img.transform(a, b, c, d, e, f).map_err(lib)?;
        // As shown: turned a quarter for orientations 5..=8.
        let (w, h) = if o >= 5 { (h, w) } else { (w, h) };
        let (pw, ph) = match size {
            Some(s) => (s.width, s.height),
            None if w.max(h) > *SIDE.end() => {
                return Err(PdfiumError::InvalidInput(format!(
                    "{what} is {w}x{h} px: as a page at 1 pt per pixel it is over 14400 pt; \
                     choose a page size"
                )));
            }
            None => (w, h),
        };
        let s = (pw / w).min(ph / h);
        img.scale(s, s).map_err(lib)?;
        let (dx, dy) = ((pw - w * s) / 2.0, (ph - h * s) / 2.0);
        img.translate(PdfPoints::new(dx), PdfPoints::new(dy))
            .map_err(lib)?;
        let paper = PdfPagePaperSize::from_points(PdfPoints::new(pw), PdfPoints::new(ph));
        let mut p = doc.pages_mut().create_page_at_end(paper).map_err(lib)?;
        p.objects_mut().add_image_object(img).map_err(lib)?;
        save(&doc)
    })
}

/// Affine map `[a, b, c, d, e, f]` (PDF order: `x' = a x + c y + e`,
/// `y' = b x + d y + f`) taking an image drawn as stored over `w` x `h`
/// points (y-up) to how orientation `o` shows it, over its oriented size
/// (`h` x `w` for 5..=8).
pub(crate) fn matrix(o: u16, w: f32, h: f32) -> [f32; 6] {
    match o {
        2 => [-1.0, 0.0, 0.0, 1.0, w, 0.0],  // mirrored left-right
        3 => [-1.0, 0.0, 0.0, -1.0, w, h],   // turned 180
        4 => [1.0, 0.0, 0.0, -1.0, 0.0, h],  // mirrored top-bottom
        5 => [0.0, -1.0, -1.0, 0.0, h, w],   // transposed
        6 => [0.0, -1.0, 1.0, 0.0, 0.0, w],  // turned 90 clockwise
        7 => [0.0, 1.0, 1.0, 0.0, 0.0, 0.0], // transversed
        8 => [0.0, 1.0, -1.0, 0.0, h, 0.0],  // turned 90 counter-clockwise
        _ => [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    }
}
