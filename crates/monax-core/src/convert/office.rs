//! Office documents to PDF through the bundled WordCraft sidecar
//! (`wordcraft-cli`, R152). Missing, every conversion fails with
//! `OfficeUnavailable` before anything runs.
use crate::ProgressSink;
use crate::output::{check_out, check_out_dir, private_dir, stem, write_output};
use crate::pages::{PagesError, reason};
use monax_sidecar::Tool;
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Bound of one conversion.
const TIMEOUT: Duration = Duration::from_secs(300);
/// WordCraft's error for an encrypted document opened without a password
/// (`wordcraft_docx::PASSWORD_REQUIRED`, also given for an encrypted .doc).
const PASSWORD_REQUIRED: &str = "this document is protected with a password";
/// Office document types [`office_to_pdf`] takes, by extension.
const TYPES: [&str; 8] = ["docx", "doc", "odt", "rtf", "txt", "html", "htm", "md"];

/// WordCraft's `wordcraft-cli`, when present, from the one discovery
/// ([`monax_sidecar::find`]): `MONAX_BIN_DIR`, then the app bundle's
/// `Contents/Resources/sidecars`; never `PATH`.
pub fn office_available() -> Option<PathBuf> {
    monax_sidecar::find(Tool::Wordcraft)
}

/// Converts the document `input` (docx, doc, odt, rtf, txt, html, htm, md;
/// extension case-insensitive, checked against the file's first bytes; txt,
/// html, htm and md any bytes) to `<out_dir>/<stem>.pdf` with WordCraft
/// (copy-on-save: a taken name gets `-1`, `-2`, ...). WordCraft reads
/// `input` in place and writes into a private 0700 temp dir removed on
/// every path; the run is bounded to 300 s and cancelling `progress` kills
/// its process group. Returns the path written; a failed run writes nothing.
///
/// # Errors
/// `OfficeUnavailable` (checked first, nothing runs), `InvalidInput` (bad
/// path or `out_dir`, an unsupported or mislabelled file), `EncryptedInput`
/// (an OLE-headed .docx, or WordCraft asks for a password; R155), `Office`
/// (non-zero exit: WordCraft's last stderr line, at most 200 chars),
/// `OfficeNoOutput` (exit 0, no file), `Sidecar` (timed out, cancelled),
/// `Io`.
pub fn office_to_pdf(
    input: &Path,
    out_dir: &Path,
    progress: &ProgressSink,
) -> Result<PathBuf, PagesError> {
    office_available().ok_or(PagesError::OfficeUnavailable)?;
    check_type(input)?;
    check_out_dir(out_dir)?;
    let out = out_dir.join(format!("{}.pdf", stem(input)));
    check_out(&out, &[input])?; // before the costly run
    let tmp = private_dir()?; // removed on every path
    let made = tmp.path().join("out.pdf");
    let args: [OsString; 3] = ["convert".into(), input.into(), made.clone().into()];
    let run = monax_sidecar::run(Tool::Wordcraft, &args, TIMEOUT, progress.cancel_flag())?;
    if !run.status.success() {
        let why = reason(&run);
        if why.contains(PASSWORD_REQUIRED) {
            return Err(PagesError::EncryptedInput); // R155: no password prompt in v1
        }
        return Err(PagesError::Office(why.chars().take(200).collect()));
    }
    if !made.is_file() {
        return Err(PagesError::OfficeNoOutput);
    }
    write_output(&out, &[input], |tmp| {
        std::fs::copy(&made, tmp)
            .map(drop)
            .map_err(PagesError::from)
    })
}

/// A supported extension whose file starts as that type does.
fn check_type(input: &Path) -> Result<(), PagesError> {
    crate::check_input_file(input)?;
    let ext = input
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase());
    let Some(ext) = ext.filter(|e| TYPES.contains(&e.as_str())) else {
        return Err(PagesError::InvalidInput(format!(
            "{} is not a supported Office document ({})",
            input.display(),
            TYPES.join(", ")
        )));
    };
    let mut head = Vec::with_capacity(5);
    std::fs::File::open(input)?.take(5).read_to_end(&mut head)?;
    if starts_as(&ext, &head) {
        return Ok(());
    }
    if ext == "docx" && starts_as("doc", &head) {
        return Err(PagesError::EncryptedInput); // R155: encrypted / IRM OOXML is OLE
    }
    Err(PagesError::InvalidInput(format!(
        "{} is not a valid .{ext} file",
        input.display()
    )))
}

/// Whether `head` (the first bytes) fits the lowercase extension `ext`:
/// docx/odt a zip, doc an OLE file, rtf `{\rtf`, txt/html/htm/md anything.
fn starts_as(ext: &str, head: &[u8]) -> bool {
    match ext {
        "docx" | "odt" => head.starts_with(b"PK\x03\x04"),
        "doc" => head.starts_with(b"\xD0\xCF\x11\xE0"),
        "rtf" => head.starts_with(b"{\\rtf"),
        _ => true,
    }
}

#[cfg(test)]
mod tests;
