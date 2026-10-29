//! encrypt: AES-256 (or AES-128) with an owner password and permissions.
use super::secret::{check_line, secret_file};
use crate::content::qpdf_write;
use crate::pages::{PagesError, open};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// How a reader may print the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Print {
    None,
    /// Degraded (low resolution) printing only.
    Low,
    High,
}

/// What a reader without the owner password may do. Defaults: everything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Permissions {
    /// Printing: `none`, `low` (degraded) or `high` resolution.
    pub print: Print,
    /// Changes not covered by the other permissions (editing content).
    pub modify: bool,
    /// Copying or extracting text and images.
    pub extract: bool,
    /// Adding or changing comments and other annotations.
    pub annotate: bool,
    /// Filling in form fields.
    pub fill_forms: bool,
    /// Inserting, deleting and rotating pages.
    pub assemble: bool,
    /// Text extraction for accessibility tools. AES encryption (PDF 2.0)
    /// always allows it, so `false` is noted and has no effect.
    pub accessibility: bool,
}

impl Default for Permissions {
    fn default() -> Self {
        Self {
            print: Print::High,
            modify: true,
            extract: true,
            annotate: true,
            fill_forms: true,
            assemble: true,
            accessibility: true,
        }
    }
}

fn yes() -> bool {
    true
}

/// Encryption settings. The passwords are secrets: they reach qpdf only
/// through a 0600 temp file and never appear in errors, notes or logs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EncryptOpts {
    /// Secret. Password needed to open the document; empty (the default)
    /// opens without one, with the permissions still enforced.
    #[serde(default)]
    pub user_pw: crate::Secret,
    /// Secret. Password that lifts the restrictions: non-empty and
    /// different from `user_pw`.
    pub owner_pw: crate::Secret,
    /// What a reader without the owner password may do.
    #[serde(default)]
    pub permissions: Permissions,
    /// AES-256 (default); `false` uses AES-128. RC4 is never used.
    #[serde(default = "yes")]
    pub aes256: bool,
}

fn yn(b: bool) -> &'static str {
    if b { "y" } else { "n" }
}

impl EncryptOpts {
    fn check(&self) -> Result<(), PagesError> {
        let bad = |why: &str| Err(PagesError::InvalidInput(why.into()));
        check_line(self.user_pw.expose())?;
        check_line(self.owner_pw.expose())?;
        if self.owner_pw.expose().is_empty() {
            return bad("owner password is required");
        }
        if self.owner_pw == self.user_pw {
            return bad("owner password must differ from the user password");
        }
        let effective = Permissions {
            accessibility: true, // AES always allows it
            ..self.permissions
        };
        if self.user_pw.expose().is_empty() && effective == Permissions::default() {
            return bad("nothing to protect: set a user password or restrict a permission");
        }
        Ok(())
    }

    /// qpdf `--encrypt ... --` arguments, one per argfile line.
    fn args(&self) -> Vec<String> {
        let p = &self.permissions;
        let print = match p.print {
            Print::None => "none",
            Print::Low => "low",
            Print::High => "full",
        };
        let mut v = vec![
            "--encrypt".to_string(),
            format!("--user-password={}", self.user_pw.expose()),
            format!("--owner-password={}", self.owner_pw.expose()),
            format!("--bits={}", if self.aes256 { 256 } else { 128 }),
        ];
        if !self.aes256 {
            v.push("--use-aes=y".into());
        }
        v.extend([
            format!("--print={print}"),
            format!("--modify-other={}", yn(p.modify)),
            format!("--extract={}", yn(p.extract)),
            format!("--annotate={}", yn(p.annotate)),
            format!("--form={}", yn(p.fill_forms)),
            format!("--assemble={}", yn(p.assemble)),
            format!("--accessibility={}", yn(p.accessibility)),
            "--".into(),
        ]);
        v
    }

    /// What the job should tell the user: settings that have no effect.
    pub(super) fn notes(&self) -> Vec<String> {
        match self.permissions.accessibility {
            true => vec![],
            false => {
                vec!["accessibility cannot be restricted: AES encryption always allows it".into()]
            }
        }
    }
}

/// Writes `input` encrypted with `opts` to `out`: AES-256 by default
/// (AES-128 when `aes256` is false), user and owner passwords, and the
/// permissions mapped 1:1 to qpdf's. An owner-password-only input is
/// re-encrypted with the new settings. Returns the path written.
///
/// # Errors
/// `InvalidInput` (bad paths, empty owner password, owner equal to user,
/// nothing restricted, a password with a line break), `EncryptedInput`,
/// `Damaged`, `Qpdf`, `Sidecar`, `Pdfium`, `Io`.
pub fn encrypt(input: &Path, out: &Path, opts: EncryptOpts) -> Result<PathBuf, PagesError> {
    opts.check()?;
    open(input)?;
    let file = secret_file(&opts.args())?;
    let mut at = OsString::from("@");
    at.push(file.path());
    qpdf_write(input, input, vec![at], out)
}
