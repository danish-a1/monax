//! Sidecar discovery, version probing and the process runner for qpdf,
//! tesseract and wordcraft-cli.

mod runner;
mod tools;

pub use runner::{RunOutput, run, run_env};
pub use tools::Tool;

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

const VERSION_TIMEOUT: Duration = Duration::from_secs(2);

/// Failure to find, probe or run a sidecar.
#[derive(Debug, thiserror::Error)]
pub enum SidecarError {
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("{0} failed to run: {1}")]
    Spawn(&'static str, #[source] std::io::Error),
    #[error("{0} timed out")]
    Timeout(&'static str),
    #[error("{0} cancelled")]
    Cancelled(&'static str),
    #[error("{0} output exceeds {1} bytes")]
    OutputTooLarge(&'static str, u64),
    #[error("{0} exited with {1}")]
    Failed(&'static str, std::process::ExitStatus),
}

/// A tool's first `--version` line, trimmed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version(pub String);

/// Which sidecars are present and their versions.
#[derive(Debug, Clone, Default)]
pub struct Capabilities {
    pub qpdf: Option<Version>,
    pub tesseract: Option<Version>,
    pub wordcraft: Option<Version>,
}

impl Capabilities {
    /// Probes all tools; an absent or failing one is `None`.
    pub fn detect() -> Self {
        let probe = |t| version(t).ok().map(Version);
        Self {
            qpdf: probe(Tool::Qpdf),
            tesseract: probe(Tool::Tesseract),
            wordcraft: probe(Tool::Wordcraft),
        }
    }
}

fn is_file(p: &Path) -> bool {
    p.is_file()
}

/// Resolves a tool: `MONAX_BIN_DIR`, then the app bundle's
/// `Contents/Resources/sidecars`, then `PATH` (qpdf and tesseract only:
/// wordcraft-cli is always the bundled one).
pub fn find(tool: Tool) -> Option<PathBuf> {
    let bin = tool.bin();
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(d) = std::env::var_os("MONAX_BIN_DIR") {
        dirs.push(d.into());
    }
    if let Some(res) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.join("../Resources/sidecars")))
    {
        dirs.push(res);
    }
    if let Some(path) = std::env::var_os("PATH").filter(|_| tool.on_path()) {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs.into_iter().map(|d| d.join(bin)).find(|p| is_file(p))
}

/// tesseract's language data (`<code>.traineddata` files) for the
/// tesseract [`find`] resolves: `tessdata` beside it (the bundle,
/// `MONAX_BIN_DIR`), else `../share/tessdata` (Homebrew, dev). `None` when
/// tesseract or both dirs are absent.
pub fn tessdata() -> Option<PathBuf> {
    tessdata_beside(&find(Tool::Tesseract)?)
}

fn tessdata_beside(bin: &Path) -> Option<PathBuf> {
    let dir = bin.parent()?;
    [dir.join("tessdata"), dir.join("../share/tessdata")]
        .into_iter()
        .find(|d| d.is_dir())
}

/// Runs `<tool> --version` through [`run`] (2s timeout, child killed and
/// reaped on timeout) and returns the trimmed first output line.
///
/// # Errors
/// As [`run`], plus `Failed` on a non-zero exit.
pub fn version(tool: Tool) -> Result<String, SidecarError> {
    let out = run(
        tool,
        &["--version".into()],
        VERSION_TIMEOUT,
        &AtomicBool::new(false),
    )?;
    if !out.status.success() {
        return Err(SidecarError::Failed(tool.bin(), out.status));
    }
    Ok(out.stdout.lines().next().unwrap_or("").trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::tessdata_beside;

    #[test]
    fn tessdata_prefers_the_bundled_dir_then_homebrew_share() {
        let root = tempfile::tempdir().unwrap();
        let bin = root.path().join("bin/tesseract");
        assert_eq!(tessdata_beside(&bin), None);
        let share = root.path().join("bin/../share/tessdata");
        std::fs::create_dir_all(&share).unwrap();
        assert_eq!(tessdata_beside(&bin), Some(share));
        let bundled = root.path().join("bin/tessdata");
        std::fs::create_dir_all(&bundled).unwrap();
        assert_eq!(tessdata_beside(&bin), Some(bundled));
    }
}
