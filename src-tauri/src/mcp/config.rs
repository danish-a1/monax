//! `mcp.json`: `{ "url", "token" }`, the connection details an agent needs.
//! Directory 0700, file 0600, replaced atomically. Port and token persist
//! across launches so a saved `claude mcp add` keeps working.
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
struct Stored {
    url: String,
    token: String,
}

/// Directory for `mcp.json`: `config` (`MONAX_CONFIG_DIR`, a dev/test
/// override) when set, else `<home>/Library/Application Support/monax`.
/// `None` when the override is not absolute or neither is set.
pub(super) fn dir(config: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    match config {
        Some(c) => Some(PathBuf::from(c)).filter(|p| p.is_absolute()),
        None => Some(PathBuf::from(home?).join("Library/Application Support/monax")),
    }
}

/// `http://127.0.0.1:<port>/mcp`.
pub(super) fn url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/mcp")
}

fn valid_token(t: &str) -> bool {
    t.len() == 64 && t.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Stored `(port, token)`; `None` when the file is missing or corrupt.
pub(super) fn load(dir: &Path) -> Option<(u16, String)> {
    let s: Stored = serde_json::from_slice(&std::fs::read(dir.join("mcp.json")).ok()?).ok()?;
    let port = s
        .url
        .strip_prefix("http://127.0.0.1:")?
        .strip_suffix("/mcp")?;
    let port = port.parse().ok()?;
    valid_token(&s.token).then_some((port, s.token))
}

/// Writes `mcp.json` in `dir` (created 0700) via a 0600 temp file + rename.
pub(super) fn save(dir: &Path, url: &str, token: &str) -> std::io::Result<()> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?; // created 0600
    serde_json::to_writer(
        &mut tmp,
        &Stored {
            url: url.into(),
            token: token.into(),
        },
    )?;
    tmp.flush()?;
    tmp.persist(dir.join("mcp.json"))?;
    Ok(())
}

/// 32 random bytes as 64 lowercase hex chars.
pub(super) fn new_token() -> std::io::Result<String> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).map_err(std::io::Error::other)?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

/// Constant-time equality (length is not secret: tokens are fixed-size).
pub(super) fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_then_load_round_trips_with_private_modes() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().join("monax");
        let token = new_token().unwrap();
        save(&dir, &url(4242), &token).unwrap();
        assert_eq!(load(&dir), Some((4242, token)));
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&dir), 0o700);
        assert_eq!(mode(&dir.join("mcp.json")), 0o600);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "no temp left");
    }

    #[test]
    fn dir_prefers_absolute_override_then_home() {
        let os = |s: &str| Some(OsString::from(s));
        let app = "/Users/u/Library/Application Support/monax";
        assert_eq!(dir(None, os("/Users/u")), Some(PathBuf::from(app)));
        assert_eq!(dir(os("/tmp/c"), os("/Users/u")), Some("/tmp/c".into()));
        assert_eq!(dir(os("/tmp/c"), None), Some("/tmp/c".into()));
        assert_eq!(dir(os("rel"), os("/Users/u")), None);
        assert_eq!(dir(os(""), os("/Users/u")), None);
        assert_eq!(dir(None, None), None);
    }

    #[test]
    fn corrupt_or_missing_files_load_as_none() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(load(d.path()), None);
        let put = |s: &str| std::fs::write(d.path().join("mcp.json"), s).unwrap();
        put("not json");
        assert_eq!(load(d.path()), None);
        put(r#"{"url":"http://127.0.0.1:1/mcp","token":"short"}"#);
        assert_eq!(load(d.path()), None);
        let t = "a".repeat(64);
        put(&format!(r#"{{"url":"http://evil:1/mcp","token":"{t}"}}"#));
        assert_eq!(load(d.path()), None);
    }

    #[test]
    fn tokens_are_random_hex_and_compare_exactly() {
        let (a, b) = (new_token().unwrap(), new_token().unwrap());
        assert!(valid_token(&a) && a != b);
        assert!(same(a.as_bytes(), a.as_bytes()));
        assert!(!same(a.as_bytes(), b.as_bytes()));
        assert!(!same(a.as_bytes(), &a.as_bytes()[1..]));
        assert!(!same(b"", b"x"));
    }
}
