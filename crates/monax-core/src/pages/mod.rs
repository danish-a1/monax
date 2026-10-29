//! Page operations. merge, extract, delete, rotate, reorder, alternate mix and
//! the splits run through the qpdf sidecar; n-up, crop, insert blank and split
//! half edit through pdfium.
//!
//! Page indices are 0-based. Every op validates its input paths (absolute,
//! existing regular files), refuses encrypted or damaged input before writing
//! anything (qpdf ops: `open`; pdfium edits: the pdfium load), and publishes
//! its output copy-on-save: an existing file is never replaced (a taken name
//! gets `-1`, `-2`, ...), `out` may not resolve to an input, and a failed op
//! leaves no file behind. Ops return the path written.
mod edit;
mod error;
mod half;
mod inputs;
mod merge;
mod nup;
mod organize;
mod select;
mod split;
mod split_bookmarks;
mod split_inputs;

pub use edit::{Rect, crop, insert_blank};
pub use error::PagesError;
pub use half::split_half;
pub(crate) use inputs::input;
pub use inputs::{
    AlternateMixInput, CropInput, DeleteInput, ExtractInput, InsertBlankInput, MergeInput,
    NupInput, OrganizeInput, ReorderInput, RotateInput, SplitHalfInput,
};
pub(crate) use merge::join;
pub use merge::{MergeOpts, alternate_mix, merge};
pub use nup::nup;
pub use organize::{PagePlan, organize};
pub use select::{delete, extract, reorder, rotate};
pub use split::{RangeSpec, split_pages, split_size, split_text};
pub use split_bookmarks::split_bookmarks;
pub use split_inputs::{
    DEFAULT_PATTERN, SplitBookmarksInput, SplitPagesInput, SplitSizeInput, SplitTextInput,
};

use crate::output::OutputError;
use crate::paths::check_input_file;
use crate::pdfium::{self, PdfiumError};
use monax_sidecar::{RunOutput, Tool};
use std::ffi::OsString;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

// ponytail: fixed generous timeout, no cancel (ops take no cancel flag yet).
// TODO: thread the job's cancel flag through once ops run on the registry.
const QPDF_TIMEOUT: Duration = Duration::from_secs(600);

pub(crate) fn qpdf(args: &[OsString]) -> Result<RunOutput, PagesError> {
    let never = AtomicBool::new(false);
    Ok(monax_sidecar::run(Tool::Qpdf, args, QPDF_TIMEOUT, &never)?)
}

/// qpdf's last non-empty stderr line; without one, the exit code (or signal).
pub(crate) fn reason(out: &RunOutput) -> String {
    match out.stderr.lines().rev().find(|l| !l.trim().is_empty()) {
        Some(line) => line.trim().to_string(),
        None => match out.status.code() {
            Some(c) => format!("exit code {c}"),
            None => out.status.to_string(),
        },
    }
}

/// Runs qpdf and returns its stdout: exit 0 or 3 (warnings) is success. Any
/// other exit, including 2 (qpdf's code for every error, I/O failures among
/// them), is `Qpdf` with qpdf's own reason: damage is detected earlier, by the
/// pdfium load in `open`.
pub(crate) fn qpdf_out(args: &[OsString]) -> Result<String, PagesError> {
    succeeded(qpdf(args)?)
}

/// [`qpdf_out`] for a run already made.
pub(crate) fn succeeded(out: RunOutput) -> Result<String, PagesError> {
    match out.status.code() {
        Some(0 | 3) => Ok(out.stdout),
        _ => Err(PagesError::Qpdf(reason(&out))),
    }
}

/// [`qpdf_out`] for runs whose output is a file.
pub(crate) fn qpdf_op(args: &[OsString]) -> Result<(), PagesError> {
    qpdf_out(args).map(drop)
}

/// Checks the path and refuses input that needs a password (see
/// [`crate::security::encrypted`]); damaged input passes, so `repair` can run
/// on it. Returns whether the input is encrypted (owner password only).
pub(crate) fn refuse_encrypted(input: &Path) -> Result<bool, PagesError> {
    check_input_file(input)?;
    crate::security::encrypted(input, None)
}

/// [`refuse_encrypted`], then refuses damaged input (pdfium cannot load it);
/// returns the page count.
pub(crate) fn open(input: &Path) -> Result<usize, PagesError> {
    refuse_encrypted(input)?;
    Ok(pdfium::page_count(input)?)
}

/// Rejects an empty list or any index `>= count`. Typed as `PdfiumError` so
/// the pdfium edits can call it inside `with_doc`; converts to `PagesError`.
pub(crate) fn check_pages(pages: &[usize], count: usize) -> Result<(), PdfiumError> {
    if pages.is_empty() {
        return Err(PdfiumError::InvalidInput("no pages given".into()));
    }
    match pages.iter().find(|&&p| p >= count) {
        Some(&page) => Err(PdfiumError::PageOutOfRange { page, count }),
        None => Ok(()),
    }
}

/// qpdf page list: 0-based indices to a 1-based comma list ("3,1,1").
pub(crate) fn page_list(pages: &[usize]) -> OsString {
    let v: Vec<String> = pages.iter().map(|p| (p + 1).to_string()).collect();
    v.join(",").into()
}

/// Publishes `bytes` (produced after the pdfium lock is released) as `out`
/// (see the module docs), in the caller's error type.
pub(crate) fn write_bytes<E: From<OutputError> + From<std::io::Error>>(
    out: &Path,
    input: &Path,
    bytes: &[u8],
) -> Result<std::path::PathBuf, E> {
    crate::output::write_output(out, &[input], |tmp| Ok::<_, E>(std::fs::write(tmp, bytes)?))
}
