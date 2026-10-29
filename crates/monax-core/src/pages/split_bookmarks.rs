//! Split at bookmarks (PDF outline entries).
use super::split::{Chunk, chunks_from_starts, publish};
use super::{PagesError, open, qpdf_out};
use crate::output::check_out_dir;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// (title, 0-based page) of every outline entry at 1-based `level`, in
/// document order. Entries without a page destination are skipped.
fn at_level(items: &Value, level: usize, out: &mut Vec<(String, usize)>) {
    for item in items.as_array().into_iter().flatten() {
        if level > 1 {
            at_level(&item["kids"], level - 1, out);
        } else if let Some(n) = item["destpageposfrom1"].as_u64() {
            let title = item["title"].as_str().unwrap_or_default().to_string();
            out.push((title, (n as usize).saturating_sub(1)));
        }
    }
}

fn outline_entries(input: &Path, level: usize) -> Result<Vec<(String, usize)>, PagesError> {
    let out = qpdf_out(&["--json".into(), "--json-key=outlines".into(), input.into()])?;
    let json: Value = serde_json::from_str(&out)
        .map_err(|e| PagesError::Qpdf(format!("unreadable outline data: {e}")))?;
    let mut entries = Vec::new();
    at_level(&json["outlines"], level, &mut entries);
    Ok(entries)
}

/// One file per bookmark at `level` (1-based; 1 = top level), each running up
/// to the next such bookmark; `%[bookmark]` is its title. Bookmarks on a page
/// already started by an earlier one are skipped. Pages before the first
/// bookmark form a leading file with an empty `%[bookmark]`.
///
/// # Errors
/// As [`super::split_pages`]; `InvalidInput` when `level` is 0 or the document
/// has no bookmarks with a page destination at that level.
pub fn split_bookmarks(
    input: &Path,
    level: usize,
    out_dir: &Path,
    pattern: &str,
) -> Result<Vec<PathBuf>, PagesError> {
    check_out_dir(out_dir)?;
    if level == 0 {
        return Err(PagesError::InvalidInput("level starts at 1".into()));
    }
    let count = open(input)?;
    let mut entries = outline_entries(input, level)?;
    entries.retain(|(_, p)| *p < count);
    entries.sort_by_key(|(_, p)| *p); // stable: first of equal pages wins
    entries.dedup_by_key(|(_, p)| *p);
    if entries.is_empty() {
        return Err(PagesError::InvalidInput(format!(
            "no bookmarks at level {level}"
        )));
    }
    let mut starts: Vec<usize> = entries.iter().map(|(_, p)| *p).collect();
    let mut titles: Vec<Option<String>> = entries.into_iter().map(|(t, _)| Some(t)).collect();
    if starts[0] != 0 {
        starts.insert(0, 0);
        titles.insert(0, None);
    }
    let mut chunks: Vec<Chunk> = chunks_from_starts(&starts, count);
    chunks
        .iter_mut()
        .zip(titles)
        .for_each(|(c, t)| c.bookmark = t);
    publish(input, out_dir, pattern, chunks)
}
