//! OCR (spec decision 4): one page at a time, pdfium renders the page as
//! displayed to a private PNG and tesseract turns it into a one-page PDF
//! (the image under an invisible text layer); qpdf joins the pages into the
//! output. Memory stays bounded by one page; the PNG goes as soon as
//! tesseract is done with it, the page PDFs with their private dir.
mod input;

pub use input::OcrInput;

use crate::ProgressSink;
use crate::output::{check_out, private_dir, private_file};
use crate::pages::{PagesError, join, open, reason};
use crate::pdfium::{Checked, encode_png, fields, page_of, px_size, render_page, with_checked};
use monax_sidecar::{SidecarError, Tool};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// Note of every OCR output (R125).
pub(crate) const NOTE: &str = "pages were replaced by images with a searchable text layer; \
    links, form fields, annotations and bookmarks were not carried over";

// ponytail: fixed per-page bound, like qpdf's; a page past it is refused.
const TIMEOUT: Duration = Duration::from_secs(600);

/// Installed OCR languages: the `<code>.traineddata` files in tesseract's
/// data dir (the bundled `tessdata`, else Homebrew's in dev; see
/// [`monax_sidecar::tessdata`]), sorted. `osd` (orientation data) is not a
/// language. Empty when tesseract or its data is absent.
pub fn languages() -> Vec<String> {
    monax_sidecar::tessdata().map_or_else(Vec::new, |d| installed(&d))
}

fn installed(dir: &Path) -> Vec<String> {
    let entries = std::fs::read_dir(dir).into_iter().flatten().flatten();
    let mut v: Vec<String> = entries
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            Some(name.strip_suffix(".traineddata")?.to_string())
        })
        .filter(|c| valid(c) && c != "osd")
        .collect();
    v.sort();
    v
}

/// A tesseract code names a file in the data dir: no paths, no `+`.
fn valid(code: &str) -> bool {
    !code.is_empty() && code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// `lang` joined for tesseract (`eng+deu`) and the data dir holding them.
fn check_langs(lang: &[String]) -> Result<(String, PathBuf), PagesError> {
    if lang.is_empty() {
        return Err(PagesError::InvalidInput("no OCR language given".into()));
    }
    if let Some(bad) = lang.iter().find(|c| !valid(c)) {
        return Err(PagesError::InvalidInput(format!(
            "OCR language code {bad:?} may hold only letters, digits and _"
        )));
    }
    monax_sidecar::find(Tool::Tesseract).ok_or(SidecarError::NotFound("tesseract"))?;
    let dir = monax_sidecar::tessdata();
    let have = dir.as_deref().map(installed).unwrap_or_default();
    match (lang.iter().find(|c| !have.contains(c)), dir) {
        (None, Some(dir)) => Ok((lang.join("+"), dir)),
        (missing, _) => Err(PagesError::MissingLanguage(
            missing.unwrap_or(&lang[0]).clone(),
        )),
    }
}

/// Writes `input` to `out` with every page replaced by an image of itself
/// as displayed (/Rotate and CropBox applied, annotations and form fields
/// drawn), rendered at `dpi` (72..=600), under an invisible text layer of
/// what tesseract read in languages `lang` (codes such as `eng`; see
/// [`languages`]). Each output page is unrotated, of the input page's
/// displayed size (within 72/`dpi` points). Links, form fields, annotations
/// and bookmarks are not carried over (the job's note says so); a page
/// with no text read is kept as its image. `progress` gets a share per
/// page; cancelling it kills a running tesseract. Returns the path written.
///
/// # Errors
/// `InvalidInput` (bad paths, no language, a code that is not letters,
/// digits and `_`, `dpi` out of range, a page too large to render),
/// `MissingLanguage` (the first code without data), `Sidecar` (tesseract or
/// qpdf not found, timed out, cancelled), `Tesseract` (it failed; its
/// reason), `EncryptedInput`, `Damaged`, `Qpdf`, `Pdfium` (the form field
/// check of [`crate::pdfium::render`]), `Io`.
pub fn run(
    input: &Path,
    out: &Path,
    lang: &[String],
    dpi: u32,
    progress: &ProgressSink,
) -> Result<PathBuf, PagesError> {
    if !(72..=600).contains(&dpi) {
        return Err(PagesError::InvalidInput(format!(
            "dpi {dpi} not in 72..=600"
        )));
    }
    let tess = check_langs(lang)?;
    let count = open(input)?;
    check_out(out, &[input])?; // before the costly pages
    let ok = fields::check(input, None)?;
    let dir = private_dir()?; // document data: private, removed on every path
    let mut parts = Vec::with_capacity(count);
    // One load per page: the pdfium lock is free while tesseract runs.
    for i in 0..count {
        if progress.is_cancelled() {
            return Err(SidecarError::Cancelled("tesseract").into());
        }
        let page = read_page(&ok, i, dpi, dir.path(), &tess, progress.cancel_flag())?;
        parts.push(page);
        progress.set((i + 1) as f32 / (count + 1) as f32);
    }
    let path = join(&parts, out, &[input])?;
    progress.note(NOTE);
    Ok(path)
}

/// Renders page `i` to `<dir>/<i>.png` (0600), has tesseract write
/// `<dir>/<i>.pdf` from it, removes the PNG; returns the PDF's path.
fn read_page(
    ok: &Checked,
    i: usize,
    dpi: u32,
    dir: &Path,
    (lang, data): &(String, PathBuf),
    cancel: &AtomicBool,
) -> Result<PathBuf, PagesError> {
    let r = with_checked(ok, None, |_, doc| {
        let p = page_of(doc, i)?;
        let (w, h) = px_size(p.width().value, p.height().value, dpi)?;
        render_page(&p, w, h, ok)
    })?;
    let png = dir.join(format!("{i}.png"));
    private_file(&png)?;
    std::fs::write(&png, encode_png(r.w, r.h, &r.rgba)?)?;
    drop(r);
    let base = dir.join(i.to_string());
    let pdf = base.with_extension("pdf");
    private_file(&pdf)?;
    // tessedit_create_pdf, not the `pdf` config file: the bundle ships no
    // tessdata/configs.
    let args: Vec<OsString> = vec![
        png.clone().into(),
        base.into(),
        "-l".into(),
        lang.into(),
        "--dpi".into(),
        dpi.to_string().into(),
        "-c".into(),
        "tessedit_create_pdf=1".into(),
    ];
    let env = [("TESSDATA_PREFIX", data.as_os_str())];
    let run = monax_sidecar::run_env(Tool::Tesseract, &args, &env, TIMEOUT, cancel);
    let _ = std::fs::remove_file(&png); // the dir goes on every path anyway
    let run = run?;
    // tesseract may exit 0 without writing its output.
    if !run.status.success() || std::fs::metadata(&pdf)?.len() == 0 {
        return Err(PagesError::Tesseract(reason(&run)));
    }
    Ok(pdf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_lists_valid_codes_without_osd() {
        let d = tempfile::tempdir().unwrap();
        for f in [
            "eng.traineddata",
            "osd.traineddata",
            "chi_sim.traineddata",
            "a+b.traineddata",
            "x.txt",
        ] {
            std::fs::write(d.path().join(f), b"").unwrap();
        }
        assert_eq!(installed(d.path()), ["chi_sim", "eng"]);
        assert!(installed(&d.path().join("none")).is_empty());
        assert!(valid("chi_sim") && !valid("") && !valid("../eng") && !valid("e.g"));
    }
}
