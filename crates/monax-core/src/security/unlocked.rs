//! Opening protected inputs for ops whose core fn takes a path: every
//! encrypted input (opened with its password, or restricted by an owner
//! password only) is decrypted to a private temp copy that the op reads
//! instead. No output carries the input's encryption, and the job says so.
use super::aliases::aliases;
use super::secret::{decrypt_into, encrypted};
use crate::Secret;
use crate::check_input_file;
use crate::output::{check_out, private_dir, private_file};
use crate::pages::PagesError;
use crate::pdfium::PdfiumError;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Note of every job that read an input opened with its password.
pub(crate) const NOT_PROTECTED: &str = "output is not password-protected";

struct Swap {
    orig: PathBuf,
    copy: PathBuf,
    _dir: TempDir,
}

impl Drop for Swap {
    fn drop(&mut self) {
        aliases().retain(|(c, _)| c != &self.copy);
    }
}

/// Decrypted temp copies of an op's inputs and the notes they call for;
/// the copies are removed when dropped, on every path.
pub(crate) struct Unlocked {
    swaps: Vec<Swap>,
    notes: Vec<String>,
}

impl Unlocked {
    /// Probes every input: with its password through qpdf, else through
    /// pdfium (qpdf only decrypts what pdfium reports encrypted). Each encrypted one is decrypted into a private temp dir
    /// (0700) as a 0600 copy with the same file name (so `%[filename]` and
    /// default names still name the original) and `path` points at it; a
    /// password for an unencrypted file is ignored. `out` is checked against
    /// the originals before anything is decrypted.
    ///
    /// # Errors
    /// `InvalidInput` (bad path or `out`, password with a line break),
    /// `EncryptedInput`, `WrongPassword`, `Qpdf`, `Sidecar`, `Io`.
    pub(crate) fn new(
        inputs: Vec<(&mut PathBuf, Option<Secret>)>,
        out: Option<&Path>,
    ) -> Result<Self, PagesError> {
        let mut locked = Vec::new();
        for (path, pw) in inputs {
            let pw = pw.as_ref().map(Secret::expose);
            let enc = match pw {
                Some(p) => {
                    check_input_file(path)?;
                    encrypted(path, Some(p))?
                }
                // No password: pdfium tells plain from owner-only in process,
                // so pdfium-only ops on plain files never need qpdf (R46).
                None => match crate::pdfium::has_security(path) {
                    Ok(enc) => enc,
                    Err(PdfiumError::Damaged) => false, // the op reports it (repair reads it)
                    Err(e) => return Err(e.into()),
                },
            };
            if enc {
                locked.push((path, pw.map(String::from)));
            }
        }
        if let (Some(out), false) = (out, locked.is_empty()) {
            let originals: Vec<&Path> = locked.iter().map(|(p, _)| p.as_path()).collect();
            check_out(out, &originals)?;
        }
        let mut this = Self {
            swaps: vec![],
            notes: vec![],
        };
        for (path, pw) in locked {
            let dir = private_dir()?;
            let copy = dir.path().join(path.file_name().unwrap_or_default());
            private_file(&copy)?;
            decrypt_into(path, pw.as_deref(), &copy)?;
            let note = match pw {
                Some(_) => NOT_PROTECTED.to_string(),
                None => format!("permissions of {} were not carried over", path.display()),
            };
            if !this.notes.contains(&note) {
                this.notes.push(note);
            }
            let orig = std::mem::replace(path, copy.clone());
            aliases().push((copy.clone(), orig.clone()));
            this.swaps.push(Swap {
                orig,
                copy,
                _dir: dir,
            });
        }
        Ok(this)
    }

    /// Drops the notes: for an op whose output is protected anew (encrypt).
    pub(crate) fn quiet(mut self) -> Self {
        self.notes.clear();
        self
    }

    /// Opens one `input` (see [`Unlocked::new`]) and runs `op` on the path to
    /// read; for the ops whose input struct is not built by `input!`.
    pub(crate) fn single<T>(
        input: &Path,
        password: Option<crate::Secret>,
        out: Option<&Path>,
        op: impl FnOnce(&Path) -> Result<T, PagesError>,
    ) -> Result<(T, Vec<String>), PagesError> {
        let mut path = input.to_path_buf();
        let unlocked = Self::new(vec![(&mut path, password)], out)?;
        unlocked.finish(op(&path).map(|v| (v, Vec::new())))
    }

    /// `r` with the temp paths in its message replaced by the originals and,
    /// the notes of [`Unlocked::new`] added to its own.
    pub(crate) fn finish<T>(
        self,
        r: Result<(T, Vec<String>), PagesError>,
    ) -> Result<(T, Vec<String>), PagesError> {
        match r {
            Ok((v, mut notes)) => {
                notes.extend(self.notes.iter().cloned());
                Ok((v, notes))
            }
            Err(e) => Err(self.restore(e)),
        }
    }

    fn restore(&self, e: PagesError) -> PagesError {
        let fix = |s: String| {
            self.swaps.iter().fold(s, |s, w| {
                s.replace(&*w.copy.to_string_lossy(), &w.orig.to_string_lossy())
            })
        };
        match e {
            PagesError::InvalidInput(s) => PagesError::InvalidInput(fix(s)),
            PagesError::Qpdf(s) => PagesError::Qpdf(fix(s)),
            e => e,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_name_the_original_never_the_temp_copy() {
        let dir = tempfile::tempdir().unwrap();
        let copy = dir.path().join("a.pdf");
        let u = Unlocked {
            swaps: vec![Swap {
                orig: "/docs/a.pdf".into(),
                copy: copy.clone(),
                _dir: dir,
            }],
            notes: vec![NOT_PROTECTED.into()],
        };
        let at = |s: &str| format!("{}{s}", copy.display());
        let e = u.restore(PagesError::Qpdf(at(": bad xref")));
        assert_eq!(e.to_string(), "qpdf failed: /docs/a.pdf: bad xref");
        let e = u.restore(PagesError::InvalidInput(at("")));
        assert_eq!(e.to_string(), "invalid input: /docs/a.pdf");
        assert!(matches!(
            u.restore(PagesError::Damaged),
            PagesError::Damaged
        ));
        let (_, notes) = u.finish(Ok(((), vec!["n".into()]))).unwrap();
        assert_eq!(notes, ["n", NOT_PROTECTED]);
    }
}
