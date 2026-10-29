//! Copy-on-save output writing shared by every operation.
use crate::names::unique_path;
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Why an output could not be written.
#[derive(Debug, thiserror::Error)]
pub(crate) enum OutputError {
    #[error("{0}")]
    InvalidInput(String),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}

/// Validates `out`: absolute, names a file (not an existing directory), its
/// parent is an existing directory, and it does not resolve (symlinks and `..`
/// followed) to any of `inputs`.
pub(crate) fn check_out(out: &Path, inputs: &[&Path]) -> Result<(), OutputError> {
    let bad = |why: &str| {
        Err(OutputError::InvalidInput(format!(
            "{why}: {}",
            out.display()
        )))
    };
    let (Some(parent), Some(name)) = (out.parent(), out.file_name()) else {
        return bad("output must name a file");
    };
    if !out.is_absolute() {
        return bad("output path must be absolute");
    }
    if out.is_dir() {
        return bad("output is a directory");
    }
    let Ok(dir) = parent.canonicalize().map(|d| d.join(name)) else {
        return bad("output directory does not exist");
    };
    if !parent.is_dir() {
        return bad("output parent is not a directory");
    }
    let resolved = dir.canonicalize().unwrap_or(dir);
    // A decrypted temp copy stands for its original (see `security::Unlocked`).
    let originals = inputs
        .iter()
        .filter_map(|i| crate::security::original_of(i));
    let all: Vec<PathBuf> = inputs
        .iter()
        .map(|i| i.to_path_buf())
        .chain(originals)
        .collect();
    for input in all {
        if input.canonicalize().is_ok_and(|i| i == resolved) {
            return bad("output would overwrite an input");
        }
    }
    Ok(())
}

/// Checks `out` against `inputs`, has `write` fill a fresh temp file in `out`'s
/// directory, then publishes it without ever replacing an existing file: a
/// taken name becomes `stem-1.ext`, `stem-2.ext`, ... Returns the path written.
/// On any failure the temp file is removed, so no partial output remains.
///
/// # Errors
/// `InvalidInput` when `out` fails the checks; whatever `write` returns; `Io`.
pub(crate) fn write_output<E: From<OutputError>>(
    out: &Path,
    inputs: &[&Path],
    write: impl FnOnce(&Path) -> Result<(), E>,
) -> Result<PathBuf, E> {
    check_out(out, inputs)?;
    let dir = out.parent().unwrap_or(out);
    let mut tmp = tempfile::Builder::new()
        .prefix(".monax-")
        .suffix(".tmp")
        .permissions(std::fs::Permissions::from_mode(0o666)) // umask applies, like a normal file
        .tempfile_in(dir)
        .map_err(OutputError::from)?;
    write(tmp.path())?;
    // ponytail: bounded retries only guard against a pathological rename race.
    for _ in 0..64 {
        let target = unique_path(out.to_path_buf());
        match tmp.persist_noclobber(&target) {
            Ok(_) => return Ok(target),
            Err(e) if e.error.kind() == ErrorKind::AlreadyExists => tmp = e.file,
            Err(e) => return Err(OutputError::Io(e.error).into()),
        }
    }
    Err(OutputError::Io(ErrorKind::AlreadyExists.into()).into())
}

/// A temp dir only the user can enter (0700), for copies of document data;
/// removed when dropped.
pub(crate) fn private_dir() -> std::io::Result<tempfile::TempDir> {
    let mode = std::fs::Permissions::from_mode(0o700);
    tempfile::Builder::new().permissions(mode).tempdir()
}

/// Creates (or empties) `p` as a file only the user can read (0600), so
/// whatever is written to it next, by us or a sidecar, stays private.
pub(crate) fn private_file(p: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::File::options()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(p)
        .map(drop)
}

/// Fails with `InvalidInput` unless `dir` is an absolute path to an existing
/// directory.
pub(crate) fn check_out_dir(dir: &Path) -> Result<(), OutputError> {
    if dir.is_absolute() && dir.is_dir() {
        return Ok(());
    }
    Err(OutputError::InvalidInput(format!(
        "output directory must be an absolute path to an existing directory: {}",
        dir.display()
    )))
}

/// File name of `path` without its extension, lossily decoded; empty if none.
pub(crate) fn stem(path: &Path) -> String {
    path.file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned())
}

/// Best-effort removal of already published `paths`; errors are ignored.
pub(crate) fn remove_all(paths: &[PathBuf]) {
    for p in paths {
        let _ = std::fs::remove_file(p);
    }
}

/// `out_dir`, else `input`'s directory (multi-output ops). Validates `input`
/// first so a bad input path reports itself, not a bad default directory.
pub(crate) fn dir_or_beside<E: From<crate::PathError>>(
    out_dir: &Option<PathBuf>,
    input: &Path,
) -> Result<PathBuf, E> {
    crate::check_input_file(input)?;
    Ok(out_dir
        .clone()
        .unwrap_or_else(|| input.parent().unwrap_or(input).to_path_buf()))
}
