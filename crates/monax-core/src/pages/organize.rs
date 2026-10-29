//! Organize: delete, reorder and rotate pages in one output.
use super::{PagesError, check_pages, open, page_list, qpdf_op};
use crate::output::write_output;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// One output page: the 0-based source `page` and a clockwise `rotate` in
/// degrees (multiple of 90, 0 allowed, negative is counter-clockwise),
/// relative to the page's current /Rotate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PagePlan {
    /// Source page, 0-based.
    pub page: usize,
    /// Clockwise degrees, a multiple of 90 (0 allowed, negative is
    /// counter-clockwise), relative to the page's current rotation.
    pub rotate: i16,
}

/// Writes one page to `out` per `plan` entry, in plan order, rotated as
/// planned. Pages absent from the plan are deleted; one source page may appear
/// more than once. A single qpdf pass. Returns the path written.
///
/// # Errors
/// `InvalidInput` (bad paths, empty `plan`, rotation not a multiple of 90),
/// `PageOutOfRange`, `EncryptedInput`, `Damaged`, `Qpdf`, `Sidecar`, `Pdfium`,
/// `Io`.
pub fn organize(input: &Path, plan: &[PagePlan], out: &Path) -> Result<PathBuf, PagesError> {
    if let Some(p) = plan.iter().find(|p| p.rotate % 90 != 0) {
        return Err(PagesError::InvalidInput(format!(
            "rotation must be a multiple of 90, got {}",
            p.rotate
        )));
    }
    let count = open(input)?;
    let pages: Vec<usize> = plan.iter().map(|p| p.page).collect();
    check_pages(&pages, count)?;
    // qpdf --rotate page numbers address the output file.
    let mut args: Vec<OsString> = vec![
        input.into(),
        "--pages".into(),
        ".".into(),
        page_list(&pages),
        "--".into(),
    ];
    for deg in [90, 180, 270] {
        let at: Vec<usize> = (0..plan.len())
            .filter(|&i| plan[i].rotate.rem_euclid(360) == deg)
            .collect();
        if !at.is_empty() {
            let mut a = OsString::from(format!("--rotate=+{deg}:"));
            a.push(page_list(&at));
            args.push(a);
        }
    }
    write_output(out, &[input], |tmp| {
        let mut all = args.clone();
        all.push(tmp.into());
        qpdf_op(&all)
    })
}
