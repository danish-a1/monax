//! Serde input structs of the security commands (see `pages::inputs`).
//! decrypt, unlock and inspect take their passwords themselves, so their
//! inputs are not opened first; encrypt opens its input like any op.
use super::EncryptOpts;
use super::Redaction;
use crate::pages::{PagesError, input, write_bytes};
use std::path::PathBuf;

input!(@full /// Writes an encrypted copy of a PDF: AES-256 by default, a user
    /// password to open it (optional) and an owner password that lifts the
    /// permissions. Passwords are secrets, never logged or echoed.
    EncryptInput { /// Absolute path of the source PDF.
        input: PathBuf, /// Passwords, permissions and method.
        opts: EncryptOpts,
        /// Secret: password (user or owner) that opens an encrypted `input`
        /// to protect it anew. Never logged or echoed.
        #[serde(default)] password: Option<crate::Secret> }
    "encrypt", |s, out| &s.input => super::encrypt(&s.input, &out, s.opts.clone())
        .map(|p| (p, s.opts.notes())), unlock quiet |u| vec![(&mut u.input, u.password.take())]);
input!(@full /// Writes a copy of a password-protected PDF with all
    /// encryption removed.
    DecryptInput { /// Absolute path of the encrypted PDF.
        input: PathBuf,
        /// Secret: the user or owner password. Never logged or echoed.
        password: crate::Secret }
    "decrypt", |s, out| &s.input => super::decrypt(&s.input, &out, s.password.expose())
        .map(|p| (p, Vec::new())), unlock |_u| vec![]);
input!(@full /// Writes a copy of a PDF that opens without a password but
    /// restricts printing, copying or editing, with the restrictions removed.
    /// A PDF that needs a password to open fails: decrypt it instead.
    UnlockInput { /// Absolute path of the restricted PDF.
        input: PathBuf }
    "unlock", |s, out| &s.input => super::unlock(&s.input, &out)
        .map(|p| (p, Vec::new())), unlock |_u| vec![]);

input!(/// Blacks out areas of pages for good: each page with a box is
    /// replaced by a 300 dpi image of itself with the boxes filled black, and
    /// nothing of the original page (text, fonts, images, annotations, form
    /// fields) is kept, so its text is no longer searchable or selectable.
    /// Boxes are in PDF user space: points, origin bottom-left, y-up,
    /// relative to the unrotated MediaBox.
    RedactInput { /// Absolute path of the source PDF.
        input: PathBuf, /// Areas to black out; several may share a page.
        boxes: Vec<Redaction> }
    "redact" -> notes, |s, out| &s.input => super::redact_with_notes(&s.input, &out, &s.boxes));

/// Writes a .json describing a PDF's security: `encrypted`,
/// `needs_password`, `method` (`AES-256`, `AES-128`, `RC4-128`, `RC4-40`)
/// and `permissions` (print, modify, extract, annotate, fill_forms,
/// assemble, accessibility). Without the password of a file that needs one,
/// `method` and `permissions` are null.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InspectSecurityInput {
    /// Absolute path of the PDF to inspect.
    pub input: PathBuf,
    /// Secret: user or owner password of a file that needs one. Never
    /// logged or echoed.
    #[serde(default)]
    pub password: Option<crate::Secret>,
    /// Absolute output path of the `.json`; defaults beside the input.
    #[serde(default)]
    pub out: Option<PathBuf>,
}

impl InspectSecurityInput {
    /// Writes [`super::SecurityInfo`] as JSON; returns the path written.
    ///
    /// # Errors
    /// As [`super::inspect`], plus `InvalidInput` for a bad `out`.
    pub fn run(self) -> Result<PathBuf, PagesError> {
        let out = crate::read::dest(&self.out, &self.input, "security", "json");
        let info = super::inspect(
            &self.input,
            self.password.as_ref().map(crate::Secret::expose),
        )?;
        let json = serde_json::to_vec(&info).map_err(std::io::Error::other)?;
        write_bytes(&out, &self.input, &json)
    }
}
