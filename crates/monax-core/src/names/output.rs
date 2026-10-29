use super::{NameCtx, expand, today};
use std::path::{Path, PathBuf};

/// Default GUI output path for a single-input tool: `input`'s directory plus
/// `<stem>-<tool_id>.pdf` (pattern `%[filename]-<tool_id>.pdf`, see [`expand`]).
/// Path separators in `tool_id` are replaced. Pure: no I/O, the name may be
/// taken (output helpers add `-1`, `-2`, ...).
pub fn default_output(input: &Path, tool_id: &str) -> PathBuf {
    let ctx = NameCtx {
        filename: crate::output::stem(input),
        page: 1,
        bookmark: None,
        index: 1,
        date: today(),
    };
    let pattern = format!("%[filename]-{}.pdf", tool_id.replace(['/', '\\'], "_"));
    input.with_file_name(expand(&pattern, &ctx))
}
