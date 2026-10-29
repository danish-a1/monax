//! Splits: one input into several PDFs written to an output directory.
//! Output names come from a pattern (see `names::expand`: `%[filename]`,
//! `%[index]`, `%[page]`, `%[bookmark]`, `%[date]`), extension included. If a
//! later chunk fails, every output already published is removed.
use super::select::select;
use super::{PagesError, check_pages, open, page_list, qpdf_op};
use crate::names::{NameCtx, expand, today};
use crate::output::{check_out_dir, private_dir, private_file, remove_all, stem};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Inclusive 0-based page range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RangeSpec {
    /// First page, 0-based.
    pub start: usize,
    /// Last page, 0-based, inclusive (>= `start`).
    pub end: usize,
}

/// One output file: 0-based pages in order, plus the bookmark title naming it.
pub(super) struct Chunk {
    pub pages: Vec<usize>,
    pub bookmark: Option<String>,
}

/// Chunks from sorted, distinct chunk-start pages: each runs to the next start.
pub(super) fn chunks_from_starts(starts: &[usize], count: usize) -> Vec<Chunk> {
    let ends = starts.iter().skip(1).copied().chain([count]);
    starts
        .iter()
        .zip(ends)
        .map(|(&s, e)| Chunk {
            pages: (s..e).collect(),
            bookmark: None,
        })
        .collect()
}

/// Writes each chunk to `out_dir` under its expanded name (existing files are
/// never replaced; a taken name gets `-1`, `-2`, ...). Returns paths in order.
pub(super) fn publish(
    input: &Path,
    out_dir: &Path,
    pattern: &str,
    chunks: Vec<Chunk>,
) -> Result<Vec<PathBuf>, PagesError> {
    let mut done: Vec<PathBuf> = Vec::new();
    let filename = stem(input);
    let date = today();
    for (i, c) in chunks.into_iter().enumerate() {
        let ctx = NameCtx {
            filename: filename.clone(),
            page: c.pages.first().map_or(1, |p| p + 1),
            bookmark: c.bookmark,
            index: i + 1,
            date: date.clone(),
        };
        let out = out_dir.join(expand(pattern, &ctx));
        match select(input, &c.pages, &out) {
            Ok(p) => done.push(p),
            Err(e) => {
                remove_all(&done);
                return Err(e);
            }
        }
    }
    Ok(done)
}

/// Splits `input` into one file per range (0-based, inclusive, in the order
/// given; ranges may overlap). Names: `pattern`, `%[page]` = range start (1-based).
///
/// # Errors
/// `InvalidInput` (no ranges, `start > end`, bad `out_dir`), `PageOutOfRange`,
/// `EncryptedInput`, `Damaged`, `Qpdf`, `Sidecar`, `Pdfium`, `Io`.
pub fn split_pages(
    input: &Path,
    ranges: &[RangeSpec],
    out_dir: &Path,
    pattern: &str,
) -> Result<Vec<PathBuf>, PagesError> {
    check_out_dir(out_dir)?;
    let count = open(input)?;
    if ranges.is_empty() {
        return Err(PagesError::InvalidInput("no ranges given".into()));
    }
    let mut chunks = Vec::new();
    for r in ranges {
        if r.start > r.end {
            return Err(PagesError::InvalidInput(format!(
                "range start {} is after end {}",
                r.start, r.end
            )));
        }
        let pages: Vec<usize> = (r.start..=r.end).collect();
        check_pages(&pages, count)?;
        chunks.push(Chunk {
            pages,
            bookmark: None,
        });
    }
    publish(input, out_dir, pattern, chunks)
}

/// Greedy chunks of consecutive pages, each written file at most `max_bytes`.
/// A single page larger than `max_bytes` becomes its own chunk (not an error).
/// Sizes are measured by writing candidates to a scratch dir, so this runs
/// qpdf about once per page plus once per chunk.
///
/// # Errors
/// As [`split_pages`]; `InvalidInput` when `max_bytes` is 0.
pub fn split_size(
    input: &Path,
    max_bytes: u64,
    out_dir: &Path,
    pattern: &str,
) -> Result<Vec<PathBuf>, PagesError> {
    check_out_dir(out_dir)?;
    if max_bytes == 0 {
        return Err(PagesError::InvalidInput("max size must be positive".into()));
    }
    let count = open(input)?;
    let scratch = private_dir()?; // page data
    let probe = scratch.path().join("probe.pdf");
    private_file(&probe)?;
    let mut starts = vec![0];
    let mut first = 0;
    for p in 1..count {
        let pages: Vec<usize> = (first..=p).collect();
        qpdf_op(&[
            input.into(),
            "--pages".into(),
            ".".into(),
            page_list(&pages),
            "--".into(),
            probe.as_os_str().into(),
        ])?;
        if std::fs::metadata(&probe)?.len() > max_bytes {
            starts.push(p);
            first = p;
        }
    }
    publish(input, out_dir, pattern, chunks_from_starts(&starts, count))
}

/// A new chunk starts at every page whose text contains `needle` (case
/// sensitive); pages before the first match form the first chunk. No match
/// gives one chunk with the whole document.
///
/// # Errors
/// As [`split_pages`]; `InvalidInput` when `needle` is empty.
pub fn split_text(
    input: &Path,
    needle: &str,
    out_dir: &Path,
    pattern: &str,
) -> Result<Vec<PathBuf>, PagesError> {
    check_out_dir(out_dir)?;
    if needle.is_empty() {
        return Err(PagesError::InvalidInput("search text is empty".into()));
    }
    let count = open(input)?;
    let texts = crate::pdfium::page_texts(input)?;
    // Page 0 always starts a chunk, match or not.
    let starts: Vec<usize> = (0..count)
        .filter(|&p| p == 0 || texts.get(p).is_some_and(|t| t.contains(needle)))
        .collect();
    publish(input, out_dir, pattern, chunks_from_starts(&starts, count))
}
