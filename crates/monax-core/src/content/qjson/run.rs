//! The qpdf runs behind [`super::Doc`]'s reads and the form rewrite.
use crate::output::private_file;
use crate::pages::{PagesError, qpdf, succeeded};
use crate::security::{password_arg, secret_file};
use std::ffi::OsString;
use std::path::Path;

/// Writes qpdf's rewrite of `input` (opened with `password` when given;
/// its encryption kept) to `out`, a new private (0600) file: every object
/// as qpdf reads it, a reference it cannot resolve written as null.
///
/// # Errors
/// `DamagedRepairFirst` (qpdf warns or fails reading `input`), as
/// [`super::Doc::read_intact`]; `InvalidInput` (a password with a line break),
/// `Qpdf`, `Sidecar`, `Io`.
pub(crate) fn rewrite_intact(
    input: &Path,
    out: &Path,
    password: Option<&str>,
) -> Result<(), PagesError> {
    private_file(out)?;
    run(input, password, &[], out, true)
}

/// Runs qpdf on `input` (password in a 0600 file, never argv) with
/// `flags`, writing `out`; with `intact`, exit 3 (warnings) or 2 (errors)
/// is `DamagedRepairFirst` (R112).
pub(super) fn run(
    input: &Path,
    password: Option<&str>,
    flags: &[&str],
    out: &Path,
    intact: bool,
) -> Result<(), PagesError> {
    let file = password.map(|p| secret_file(&[p.into()])).transpose()?;
    let mut args: Vec<OsString> = file.iter().map(password_arg).collect();
    args.push(input.into());
    args.extend(flags.iter().map(OsString::from));
    args.extend(["--".into(), out.as_os_str().into()]);
    let out = qpdf(&args)?;
    // The exit code, not stderr: the runner keeps only its tail.
    if intact && matches!(out.status.code(), Some(2 | 3)) {
        return Err(PagesError::DamagedRepairFirst);
    }
    succeeded(out).map(drop)
}
