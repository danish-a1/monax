//! Embedded image extraction to PNG.
use super::thumb::encode;
use super::{PdfiumError, Raster, page_of, with_doc};
use crate::output::{check_out_dir, dir_or_beside, remove_all, stem, write_output};
use crate::pages::PagesError;
use crate::security::Unlocked;
use pdfium_render::prelude::PdfPageObjectsCommon;
use std::path::{Path, PathBuf};

fn page_images(path: &Path, page: usize) -> Result<Vec<Raster>, PdfiumError> {
    with_doc(path, |_, doc| {
        let p = page_of(doc, page)?;
        let mut out = Vec::new();
        for obj in p.objects().iter() {
            if let Some(img) = obj.as_image_object() {
                let bmp = img
                    .get_processed_bitmap(doc)
                    .map_err(|_| PdfiumError::Damaged)?;
                out.push(Raster {
                    w: bmp.width() as u32,
                    h: bmp.height() as u32,
                    rgba: bmp.as_rgba_bytes(),
                });
            }
        }
        Ok(out)
    })
}

/// Encodes `img` and publishes it copy-on-save as `wanted` (see `write_output`).
fn write_png(input: &Path, wanted: &Path, img: &Raster) -> Result<PathBuf, PdfiumError> {
    let bytes = encode(img.w, img.h, &img.rgba)?;
    write_output(wanted, &[input], |tmp| {
        Ok::<_, PdfiumError>(std::fs::write(tmp, &bytes)?)
    })
}

/// Writes every embedded image (with filters and masks applied) as an RGBA PNG
/// into `out_dir`, named `<stem>-p<page>-<n>.png` where `page` is 1-based and `n`
/// counts images on that page from 1. Existing files are never overwritten; a
/// taken name gets a `-1`, `-2`... suffix. Returns the written paths in order.
/// On any failure every file written so far is removed.
///
/// # Errors
/// `InvalidInput` (bad path, `out_dir` not an existing absolute directory),
/// `Damaged`, `EncryptedInput`, `Io`, `Library`.
pub fn extract_images(path: &Path, out_dir: &Path) -> Result<Vec<PathBuf>, PdfiumError> {
    check_out_dir(out_dir)?;
    let count = super::page_count(path)?;
    write_all(path, out_dir, count, |page| page_images(path, page))
}

/// Writes every embedded image of a PDF (filters and masks applied) as an
/// RGBA PNG named `<stem>-p<page>-<n>.png` (`page` 1-based, `n` counts images
/// on that page from 1). A taken name gets `-1`, `-2`, ...; no images, no files.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExtractImagesInput {
    /// Absolute path of the source PDF.
    pub input: PathBuf,
    /// Absolute path of an existing output directory; defaults to the input's
    /// directory.
    #[serde(default)]
    pub out_dir: Option<PathBuf>,
    /// Secret: password (user or owner) that opens an encrypted `input`.
    /// Never logged or echoed.
    #[serde(default)]
    pub password: Option<crate::Secret>,
}

impl ExtractImagesInput {
    /// Runs [`extract_images`]; returns the files written, in order, and
    /// the job's notes.
    ///
    /// # Errors
    /// As [`extract_images`], plus `WrongPassword`.
    pub fn run(self) -> Result<(Vec<PathBuf>, Vec<String>), PagesError> {
        let dir = dir_or_beside::<PagesError>(&self.out_dir, &self.input)?;
        check_out_dir(&dir)?;
        Unlocked::single(&self.input, self.password, None, |input| {
            Ok(extract_images(input, &dir)?)
        })
    }
}

/// Writes the images of pages `0..count` (from `source`); removes every file
/// written so far if any page or write fails.
fn write_all(
    input: &Path,
    out_dir: &Path,
    count: usize,
    mut source: impl FnMut(usize) -> Result<Vec<Raster>, PdfiumError>,
) -> Result<Vec<PathBuf>, PdfiumError> {
    let stem = stem(input);
    let mut written: Vec<PathBuf> = Vec::new();
    let result = (0..count).try_for_each(|page| {
        for (n, img) in source(page)?.iter().enumerate() {
            let name = format!("{stem}-p{}-{}.png", page + 1, n + 1);
            written.push(write_png(input, &out_dir.join(name), img)?);
        }
        Ok(())
    });
    match result {
        Ok(()) => Ok(written),
        Err(e) => {
            remove_all(&written);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> Raster {
        Raster {
            w: 1,
            h: 1,
            rgba: vec![1, 2, 3, 4],
        }
    }

    fn bad() -> Raster {
        // Buffer too short for 2x2 RGBA: the PNG encoder rejects it.
        Raster {
            w: 2,
            h: 2,
            rgba: vec![0; 3],
        }
    }

    #[test]
    fn write_png_removes_its_partial_file_on_error() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("d.pdf");
        assert!(write_png(&input, &dir.path().join("a.png"), &bad()).is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn failure_on_later_image_removes_earlier_outputs() {
        let dir = tempfile::tempdir().unwrap();
        let r = write_all(&dir.path().join("d.pdf"), dir.path(), 2, |page| {
            Ok(if page == 0 {
                vec![good(), good()]
            } else {
                vec![bad()]
            })
        });
        assert!(r.is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn success_keeps_all_outputs() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("d.pdf");
        let r = write_all(&input, dir.path(), 1, |_| Ok(vec![good(), good()])).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }
}
