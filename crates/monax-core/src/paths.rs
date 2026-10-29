//! Shared validation of caller-supplied input paths.
use std::path::Path;

/// Why an input path was rejected.
#[derive(Debug, thiserror::Error)]
#[allow(clippy::enum_variant_names)] // names read as the checks they fail
pub enum PathError {
    #[error("path must be absolute: {0}")]
    NotAbsolute(String),
    #[error("file does not exist: {0}")]
    NotFound(String),
    #[error("not a regular file: {0}")]
    NotRegularFile(String),
}

/// Checks that `path` is absolute, exists and is a regular file (symlinks are
/// followed, so a link to a regular file passes). Reads metadata only.
///
/// # Errors
/// [`PathError`] naming the first failed check.
pub fn check_input_file(path: &Path) -> Result<(), PathError> {
    let shown = || path.display().to_string();
    if !path.is_absolute() {
        return Err(PathError::NotAbsolute(shown()));
    }
    let meta = std::fs::metadata(path).map_err(|_| PathError::NotFound(shown()))?;
    if !meta.is_file() {
        return Err(PathError::NotRegularFile(shown()));
    }
    Ok(())
}
