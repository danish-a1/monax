//! Live decrypted temp copies and the originals they stand for, so output
//! checks compare against the original wherever an op writes.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// (copy, original) of every live copy.
// ponytail: process-wide list of live copies (a few entries); pass the
// originals explicitly if ops ever take a context argument.
static ALIASES: Mutex<Vec<(PathBuf, PathBuf)>> = Mutex::new(Vec::new());

pub(super) fn aliases() -> MutexGuard<'static, Vec<(PathBuf, PathBuf)>> {
    ALIASES.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The original input `path` is a live temp copy of, if any.
pub(crate) fn original_of(path: &Path) -> Option<PathBuf> {
    aliases()
        .iter()
        .find(|(c, _)| c == path)
        .map(|(_, o)| o.clone())
}
