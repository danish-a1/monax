//! Secrets for qpdf: passwords reach it only through 0600 temp files
//! (`--password-file`, `@argfile`), never argv, which every local user can
//! read with `ps`.
use crate::pages::{PagesError, qpdf, reason};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::Path;
use tempfile::NamedTempFile;

/// qpdf reads its files line by line, so a secret is one line. The message
/// never echoes the secret.
pub(super) fn check_line(secret: &str) -> Result<(), PagesError> {
    if secret.contains(['\n', '\r', '\0']) {
        return Err(PagesError::InvalidInput(
            "password must not contain line breaks or NUL".into(),
        ));
    }
    Ok(())
}

/// A temp file (mode 0600, tempfile's default) holding `lines`, one per
/// line; removed when dropped, on every path.
///
/// # Errors
/// `InvalidInput` (a line break inside a line), `Io`.
pub(crate) fn secret_file(lines: &[String]) -> Result<NamedTempFile, PagesError> {
    lines.iter().try_for_each(|l| check_line(l))?;
    let mut f = NamedTempFile::new()?;
    f.write_all(lines.join("\n").as_bytes())?;
    f.flush()?;
    Ok(f)
}

/// `--password-file=<f>`.
pub(crate) fn password_arg(f: &NamedTempFile) -> OsString {
    let mut a = OsString::from("--password-file=");
    a.push(f.path());
    a
}

/// Whether `input` is encrypted, opening it with `password` when given
/// (`qpdf --requires-password`: exit 0 needs a password, 3 opens, 2 is not
/// encrypted (or unreadable: damage is left to the pdfium load) unless the
/// damaged file's raw bytes hold an encryption dictionary; any other exit is
/// qpdf failing, `Qpdf` with its reason).
/// An owner-password-only file opens without a password. The one mapping of
/// qpdf's answer to `EncryptedInput` / `WrongPassword`.
///
/// # Errors
/// `EncryptedInput` (a password is needed, none given), `WrongPassword`
/// (given and rejected), `DamagedEncrypted` (encrypted, trailer lost),
/// `InvalidInput` (password with a line break), `Qpdf`, `Sidecar`, `Io`.
pub(crate) fn encrypted(input: &Path, password: Option<&str>) -> Result<bool, PagesError> {
    let file = password.map(|p| secret_file(&[p.into()])).transpose()?;
    let mut args: Vec<OsString> = file.iter().map(password_arg).collect();
    args.extend(["--requires-password".into(), input.into()]);
    let run = qpdf(&args)?;
    match run.status.code() {
        Some(0) if password.is_some() => Err(PagesError::WrongPassword),
        Some(0) => Err(PagesError::EncryptedInput),
        Some(3) => Ok(true),
        // qpdf warns on stderr only for a damaged file; a healthy plain one
        // is never read here.
        Some(2) if !run.stderr.trim().is_empty() && looks_encrypted(input)? => {
            Err(PagesError::DamagedEncrypted)
        }
        Some(2) => Ok(false),
        _ => Err(PagesError::Qpdf(reason(&run))),
    }
}

/// Bytes read per step of the encryption scan (64 KiB).
const CHUNK: usize = 64 << 10;
/// Bytes carried from one step to the next, so a pattern split across a
/// chunk boundary is still whole: the longest pattern, `/Filter`, up to 47
/// whitespace bytes and `/Standard`, less one.
// ponytail: more whitespace than that between `/Filter` and `/Standard`,
// split across a boundary, goes unseen; widen OVERLAP if such files turn up.
const OVERLAP: usize = 63;

/// Whether the raw bytes of `input` hold an encryption dictionary: an
/// `/Encrypt` reference, or `/Filter /Standard` with `/O` and `/U` keys.
/// Streams the whole file (the trailer is at the end) in 64 KiB chunks.
fn looks_encrypted(input: &Path) -> Result<bool, PagesError> {
    Ok(scan(std::fs::File::open(input)?, CHUNK)?)
}

/// [`looks_encrypted`] over `r`, `chunk` bytes at a time; memory stays at
/// `chunk + OVERLAP` bytes.
fn scan(mut r: impl std::io::Read, chunk: usize) -> std::io::Result<bool> {
    let mut seen = [false; 4];
    let mut buf = Vec::with_capacity(chunk + OVERLAP);
    loop {
        buf.drain(..buf.len().saturating_sub(OVERLAP));
        let carried = buf.len();
        r.by_ref().take(chunk as u64).read_to_end(&mut buf)?;
        if buf.len() == carried {
            return Ok(verdict(seen));
        }
        let now = flags(&buf);
        seen = std::array::from_fn(|i| seen[i] || now[i]);
        if verdict(seen) {
            return Ok(true);
        }
    }
}

/// `[/Encrypt, /Filter /Standard, /O, /U]` found in `raw`.
fn flags(raw: &[u8]) -> [bool; 4] {
    let has = |pat: &[u8]| raw.windows(pat.len()).any(|w| w == pat);
    let standard = (0..raw.len()).any(|i| {
        raw[i..].starts_with(b"/Filter")
            && raw[i + 7..].trim_ascii_start().starts_with(b"/Standard")
    });
    [has(b"/Encrypt"), standard, has(b"/O"), has(b"/U")]
}

fn verdict([encrypt, standard, o, u]: [bool; 4]) -> bool {
    encrypt || (standard && o && u)
}

#[cfg(test)]
fn has_encryption_dict(raw: &[u8]) -> bool {
    scan(raw, CHUNK).unwrap()
}

/// `qpdf [--password-file] in --decrypt -- dest`: writes `input` without
/// encryption to `dest` (an existing file keeps its mode).
pub(super) fn decrypt_into(
    input: &Path,
    password: Option<&str>,
    dest: &Path,
) -> Result<(), PagesError> {
    let file = password.map(|p| secret_file(&[p.into()])).transpose()?;
    let mut args: Vec<OsString> = file.iter().map(password_arg).collect();
    args.extend([input.into(), "--decrypt".into(), "--".into(), dest.into()]);
    crate::pages::qpdf_op(&args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn secret_files_are_private_and_hold_one_line_each() {
        let f = secret_file(&["a b".into(), "--x=Ü".into()]).unwrap();
        let mode = std::fs::metadata(f.path()).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(std::fs::read_to_string(f.path()).unwrap(), "a b\n--x=Ü");
        let path = f.path().to_path_buf();
        drop(f);
        assert!(!path.exists());
    }

    #[test]
    fn encryption_dictionaries_are_found_in_raw_bytes() {
        assert!(has_encryption_dict(b"trailer << /Encrypt 9 0 R >>"));
        assert!(has_encryption_dict(
            b"<< /Filter  /Standard /O <a> /U <b> >>"
        ));
        assert!(!has_encryption_dict(
            b"<< /Filter /FlateDecode /Length 3 >>"
        ));
        assert!(!has_encryption_dict(b"<< /Filter /Standard >>"));
        assert!(!has_encryption_dict(b"/Filter"));
    }

    #[test]
    fn the_scan_streams_past_chunk_boundaries() {
        // Every split of each pattern across an 8-byte chunk boundary.
        for pad in 0..8 {
            let raw = [&vec![b' '; pad][..], b"<< /Encrypt 9 0 R >>"].concat();
            assert!(scan(&raw[..], 8).unwrap(), "pad {pad}");
            let raw = [&vec![b' '; pad][..], b"/Filter  /Standard /O <a> /U <b>"].concat();
            assert!(scan(&raw[..], 8).unwrap(), "pad {pad}");
        }
        // The trailer far past many chunks (as past the old 256 MiB cap).
        let mut raw = vec![b'x'; 5000];
        raw.extend(b"trailer << /Encrypt 9 0 R >>");
        assert!(scan(&raw[..], 16).unwrap());
        // /O and /U in different chunks still count together.
        let mut raw = b"/Filter /Standard /O <a>".to_vec();
        raw.extend(vec![b'x'; 300]);
        raw.extend(b"/U <b>");
        assert!(scan(&raw[..], 16).unwrap());
        assert!(!scan(&vec![b'x'; 5000][..], 16).unwrap());
    }

    #[test]
    fn multi_line_secrets_are_refused_without_echo() {
        for bad in ["se\ncret", "se\rcret", "se\0cret"] {
            let e = secret_file(&[bad.into()]).unwrap_err().to_string();
            assert_eq!(
                e,
                "invalid input: password must not contain line breaks or NUL"
            );
        }
    }
}
