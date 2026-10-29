//! Serde input structs of the multi-output page ops (the splits). Like
//! `inputs`, each struct's doc comment is its MCP tool description. Outputs go
//! to `out_dir` (default: the input's directory) named by `pattern` (default
//! [`DEFAULT_PATTERN`]).
use super::RangeSpec;
use crate::output::{check_out_dir, dir_or_beside};
use crate::security::Unlocked;
use std::path::PathBuf;

/// Default file name pattern of the split outputs.
pub const DEFAULT_PATTERN: &str = "%[filename]-%[index].pdf";

/// Declares a split input struct (fields + `out_dir` + `pattern`) whose
/// `run()` calls the op with the defaults applied.
macro_rules! split_input {
    ($(#[$m:meta])* $name:ident { $($(#[$fm:meta])* $f:ident: $t:ty),* $(,)? }
     |$s:ident, $dir:ident, $pat:ident| $run:expr) => {
        $(#[$m])*
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            /// Absolute path of the source PDF.
            pub input: PathBuf,
            $($(#[$fm])* pub $f: $t,)*
            /// Absolute path of an existing output directory; defaults to the
            /// input's directory.
            #[serde(default)]
            pub out_dir: Option<PathBuf>,
            /// Output file name pattern, extension included: `%[filename]`
            /// (input stem), `%[index]` (1-based file number), `%[page]` (first
            /// page, 1-based), `%[bookmark]`, `%[date]`. Defaults to
            /// `%[filename]-%[index].pdf`. A taken name gets `-1`, `-2`, ...
            #[serde(default)]
            pub pattern: Option<String>,
            /// Secret: password (user or owner) that opens an encrypted
            /// `input`. Never logged or echoed; the outputs are not encrypted.
            #[serde(default)]
            pub password: Option<crate::Secret>,
        }

        impl $name {
            /// Runs the split; returns the files written, in order, and the
            /// job's notes. See the matching op function for errors, plus
            /// `WrongPassword`.
            pub fn run(mut self) -> Result<(Vec<PathBuf>, Vec<String>), super::PagesError> {
                let $dir = dir_or_beside::<super::PagesError>(&self.out_dir, &self.input)?;
                check_out_dir(&$dir)?;
                let unlocked = Unlocked::new(vec![(&mut self.input, self.password.take())], None)?;
                let $pat = self.pattern.as_deref().unwrap_or(DEFAULT_PATTERN);
                let $s = &self;
                unlocked.finish(($run).map(|v| (v, Vec::new())))
            }
        }
    };
}

split_input!(/// Splits a PDF into one file per page range, in the order given
    /// (ranges may overlap).
    SplitPagesInput { /// Inclusive 0-based page ranges, one output file each.
        ranges: Vec<RangeSpec> }
    |s, dir, pat| super::split_pages(&s.input, &s.ranges, &dir, pat));
split_input!(/// Splits a PDF into runs of consecutive pages, each written file at
    /// most `max_bytes`; a single larger page becomes its own file.
    SplitSizeInput { /// Maximum size of each output file in bytes (> 0).
        max_bytes: u64 }
    |s, dir, pat| super::split_size(&s.input, s.max_bytes, &dir, pat));
split_input!(/// Splits a PDF before every page whose text contains `text` (case
    /// sensitive); pages before the first match form the first file.
    SplitTextInput { /// Text that starts a new file (non-empty).
        text: String }
    |s, dir, pat| super::split_text(&s.input, &s.text, &dir, pat));
split_input!(/// Splits a PDF into one file per bookmark at `level`, each running to
    /// the next such bookmark; `%[bookmark]` is its title. Pages before the
    /// first bookmark form a leading file.
    SplitBookmarksInput { /// Outline level, 1-based (1 = top level).
        #[serde(default = "one")] level: usize }
    |s, dir, pat| super::split_bookmarks(&s.input, s.level, &dir, pat));

fn one() -> usize {
    1
}
