//! Serde input structs, one per single-output page op. The GUI and the MCP
//! layer deserialize these verbatim, and each struct's doc comment is its MCP
//! tool description; `out` defaults to the input's directory with
//! `<stem>-<tool>.pdf` (see `names::default_output`).
use super::{MergeOpts, PagePlan, Rect};
use crate::security::with_passwords;
use std::path::PathBuf;

/// Declares an input struct (fields + optional `out`) whose `run()` opens
/// the inputs and calls the op, defaulting `out` to
/// `names::default_output(first, tool)`. `run` returns the written path and
/// the job's notes. A single-input struct gets a `password` field that opens
/// an encrypted `input` (see `security::Unlocked`); `-> notes` marks an op
/// whose call already returns `(path, notes)`. `@full` names the
/// `(path, password)` pairs to open itself (`unlock |u| ...`); `unlock quiet`
/// drops the opening notes (an op that protects its output anew).
macro_rules! input {
    (@full $(#[$m:meta])* $name:ident { $($(#[$fm:meta])* $f:ident: $t:ty),* $(,)? }
     $tool:literal, |$s:ident, $out:ident| $first:expr => $run:expr,
     unlock $($quiet:ident)? |$u:ident| $locks:expr) => {
        $(#[$m])*
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            $($(#[$fm])* pub $f: $t,)*
            /// Absolute output path; defaults to `<stem>-<tool>.pdf` beside the
            /// input. A taken name gets `-1`, `-2`, ...
            #[serde(default)]
            pub out: Option<std::path::PathBuf>,
        }

        impl $name {
            /// Runs the op; returns the path written and the job's notes. See
            /// the matching op function for errors, plus `WrongPassword`.
            pub fn run(
                mut self,
            ) -> Result<(std::path::PathBuf, Vec<String>), $crate::pages::PagesError> {
                let $out = {
                    let $s = &self;
                    $s.out
                        .clone()
                        .unwrap_or_else(|| $crate::names::default_output($first, $tool))
                };
                let unlocked = {
                    let $u = &mut self;
                    $crate::security::Unlocked::new($locks, Some(&$out))?$(.$quiet())?
                };
                let $s = &self;
                unlocked.finish($run)
            }
        }
    };
    ($(#[$m:meta])* $name:ident { $($(#[$fm:meta])* $f:ident: $t:ty),* $(,)? }
     $tool:literal -> notes, |$s:ident, $out:ident| $first:expr => $run:expr) => {
        $crate::pages::input!(@full $(#[$m])* $name {
            $($(#[$fm])* $f: $t,)*
            /// Secret: password (user or owner) that opens an encrypted
            /// `input`. Never logged or echoed; the output is not encrypted.
            #[serde(default)]
            password: Option<$crate::Secret>
        } $tool, |$s, $out| $first => $run,
          unlock |u| vec![(&mut u.input, u.password.take())]);
    };
    ($(#[$m:meta])* $name:ident { $($(#[$fm:meta])* $f:ident: $t:ty),* $(,)? }
     $tool:literal, |$s:ident, $out:ident| $first:expr => $run:expr) => {
        $crate::pages::input!($(#[$m])* $name { $($(#[$fm])* $f: $t),* }
            $tool -> notes, |$s, $out| $first => ($run).map(|p| (p, Vec::new())));
    };
}
pub(crate) use input;

input!(/// Writes the chosen pages of a PDF, in document order, to a new PDF.
    ExtractInput { /// Absolute path of the source PDF.
        input: PathBuf, /// 0-based page indices to keep (duplicates ignored).
        pages: Vec<usize> }
    "extract", |s, out| &s.input => super::extract(&s.input, &s.pages, &out));
input!(/// Writes a PDF without the chosen pages to a new PDF.
    DeleteInput { /// Absolute path of the source PDF.
        input: PathBuf, /// 0-based page indices to delete (not every page).
        pages: Vec<usize> }
    "delete", |s, out| &s.input => super::delete(&s.input, &s.pages, &out));
input!(/// Writes the pages of a PDF in a new order to a new PDF; a page may
    /// repeat and pages not listed are dropped.
    ReorderInput { /// Absolute path of the source PDF.
        input: PathBuf, /// 0-based page indices in output order.
        order: Vec<usize> }
    "reorder", |s, out| &s.input => super::reorder(&s.input, &s.order, &out));
input!(/// Rotates the chosen pages clockwise, relative to their current
    /// rotation, and writes a new PDF.
    RotateInput { /// Absolute path of the source PDF.
        input: PathBuf, /// 0-based page indices (duplicates ignored).
        pages: Vec<usize>, /// Clockwise degrees: a non-zero multiple of 90, negative is counter-clockwise.
        degrees: i16 }
    "rotate", |s, out| &s.input => super::rotate(&s.input, &s.pages, s.degrees, &out));
input!(/// Writes one page per plan entry, in plan order, each rotated as
    /// planned; pages absent from the plan are deleted.
    OrganizeInput { /// Absolute path of the source PDF.
        input: PathBuf, /// Output pages: 0-based source `page` and clockwise `rotate` (multiple of 90, 0 allowed).
        plan: Vec<PagePlan> }
    "organize", |s, out| &s.input => super::organize(&s.input, &s.plan, &out));
input!(@full /// Concatenates every page of the inputs, in order, into a new PDF; the
    /// default output sits beside the first input.
    MergeInput { /// Absolute paths of the PDFs to merge, in order.
        inputs: Vec<PathBuf>, #[serde(default)] opts: MergeOpts,
        /// Secrets: one optional password per entry of `inputs`, in order
        /// (null or missing = none). Never logged or echoed.
        #[serde(default, deserialize_with = "crate::secret::list")] #[schemars(extend("writeOnly" = true))] passwords: Vec<Option<crate::Secret>> }
    "merge", |s, out| s.inputs.first().map_or(std::path::Path::new(""), |p| p.as_path()) => {
        super::merge(&s.inputs, &out, s.opts).map(|p| (p, Vec::new()))
    }, unlock |u| with_passwords(u.inputs.iter_mut(), std::mem::take(&mut u.passwords))?);
input!(@full /// Interleaves two PDFs page by page (a1, b1, a2, b2, ...) into a new
    /// PDF, appending the longer one's leftover pages; the default output sits
    /// beside `a`.
    AlternateMixInput { /// Absolute path of the first PDF (front sides).
        a: PathBuf, /// Absolute path of the second PDF (back sides).
        b: PathBuf, /// Take `b` last page first (a back-side scan).
        #[serde(default)] reverse_b: bool,
        /// Secrets: optional passwords of `a` then `b` (null or missing =
        /// none). Never logged or echoed.
        #[serde(default, deserialize_with = "crate::secret::list")] #[schemars(extend("writeOnly" = true))] passwords: Vec<Option<crate::Secret>> }
    "alternate-mix", |s, out| &s.a => {
        super::alternate_mix(&s.a, &s.b, s.reverse_b, &out).map(|p| (p, Vec::new()))
    }, unlock |u| with_passwords([&mut u.a, &mut u.b], std::mem::take(&mut u.passwords))?);
input!(/// Tiles the pages of a PDF several to a sheet, in reading order, into a
    /// new PDF; the sheet has the first page's displayed size in points.
    NupInput { /// Absolute path of the source PDF.
        input: PathBuf, /// Pages per sheet: 2, 4, 6, 9 or 16 (2 and 6 are landscape).
        per_sheet: u8 }
    "nup", |s, out| &s.input => super::nup(&s.input, s.per_sheet, &out));
input!(/// Sets the CropBox of the chosen pages to `rect` and writes a new PDF.
    /// `rect` is in PDF user space: points, origin bottom-left, y-up, relative
    /// to the unrotated MediaBox, and must lie inside each page's MediaBox.
    CropInput { /// Absolute path of the source PDF.
        input: PathBuf, /// 0-based page indices to crop (duplicates ignored).
        pages: Vec<usize>, /// Visible area: PDF user space points, origin bottom-left, y-up, unrotated MediaBox.
        rect: Rect }
    "crop", |s, out| &s.input => super::crop(&s.input, &s.pages, s.rect, &out));
input!(/// Inserts a blank page, sized like the page it follows as displayed,
    /// after each chosen page and writes a new PDF.
    InsertBlankInput { /// Absolute path of the source PDF.
        input: PathBuf, /// 0-based page indices to insert after (duplicates ignored).
        after: Vec<usize> }
    "insert-blank", |s, out| &s.input => super::insert_blank(&s.input, &s.after, &out));
input!(/// Replaces every page with its left then right half as displayed (a
    /// two-up scan becomes single pages) and writes a new PDF.
    SplitHalfInput { /// Absolute path of the source PDF.
        input: PathBuf }
    "split-half", |s, out| &s.input => super::split_half(&s.input, &out));
