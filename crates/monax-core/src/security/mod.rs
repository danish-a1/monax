//! Security: encrypt, decrypt, unlock, inspect, redact, and opening
//! encrypted inputs for every other op (see [`crate::pages`] input structs).
//!
//! Passwords are secrets. They reach qpdf only through 0600 temp files
//! (`--password-file=<f>`, `@argfile`), never argv, and they never appear in
//! errors, notes or logs. `WrongPassword` means a password was given and
//! rejected; `EncryptedInput` that one is needed and none was given. A file
//! with only an owner password opens without one. Outputs are copy-on-save
//! as in [`crate::pages`]; errors are [`PagesError`].
mod aliases;
mod encrypt;
mod inputs;
mod inspect;
mod redact;
mod secret;
mod unlocked;

pub(crate) use aliases::original_of;
pub use encrypt::{EncryptOpts, Permissions, Print, encrypt};
pub use inputs::{DecryptInput, EncryptInput, InspectSecurityInput, RedactInput, UnlockInput};
pub use inspect::{SecurityInfo, inspect};
pub use redact::{Redaction, redact, redact_with_notes};
pub(crate) use secret::{encrypted, password_arg, secret_file};
pub(crate) use unlocked::Unlocked;

use crate::output::write_output;
use crate::pages::PagesError;
use crate::{check_input_file, pdfium};
use std::path::{Path, PathBuf};

/// Writes `input` without any encryption to `out`, opened with `password`
/// (a secret, user or owner). Returns the path written.
///
/// # Errors
/// `InvalidInput` (bad paths, input not encrypted), `WrongPassword`,
/// `Damaged`, `Qpdf`, `Sidecar`, `Pdfium`, `Io`.
pub fn decrypt(input: &Path, out: &Path, password: &str) -> Result<PathBuf, PagesError> {
    decrypt_with(input, out, Some(password))
}

/// Writes `input`, which opens without a password but carries owner
/// restrictions, to `out` with the restrictions removed. Returns the path
/// written.
///
/// # Errors
/// `InvalidInput` (bad paths, input not encrypted), `EncryptedInput` (it
/// needs a user password: use [`decrypt`]), `Damaged`, `Qpdf`, `Sidecar`,
/// `Pdfium`, `Io`.
pub fn unlock(input: &Path, out: &Path) -> Result<PathBuf, PagesError> {
    decrypt_with(input, out, None)
}

/// Refuses an unencrypted input (damaged when pdfium cannot load it), then
/// writes it decrypted.
fn decrypt_with(input: &Path, out: &Path, password: Option<&str>) -> Result<PathBuf, PagesError> {
    check_input_file(input)?;
    if !encrypted(input, password)? {
        pdfium::page_count(input)?;
        return Err(PagesError::InvalidInput("input is not encrypted".into()));
    }
    write_output(out, &[input], |tmp| {
        secret::decrypt_into(input, password, tmp)
    })
}

/// Pairs each of `paths` with its entry of `passwords` (missing = none).
///
/// # Errors
/// `InvalidInput` when there are more passwords than paths.
pub(crate) fn with_passwords<'a>(
    paths: impl IntoIterator<Item = &'a mut PathBuf>,
    passwords: Vec<Option<crate::Secret>>,
) -> Result<Vec<(&'a mut PathBuf, Option<crate::Secret>)>, PagesError> {
    let paths: Vec<_> = paths.into_iter().collect();
    if passwords.len() > paths.len() {
        return Err(PagesError::InvalidInput(
            "more passwords than inputs".into(),
        ));
    }
    let pw = passwords.into_iter().chain(std::iter::repeat(None));
    Ok(paths.into_iter().zip(pw).collect())
}
