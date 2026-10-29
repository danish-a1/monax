//! Output file-name patterns and collision-free paths.
use std::path::{Path, PathBuf};

mod output;
pub use output::default_output;

/// Values substituted into a name pattern.
#[derive(Debug, Clone)]
pub struct NameCtx {
    pub filename: String,
    pub page: usize,
    pub bookmark: Option<String>,
    pub index: usize,
    pub date: String,
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '\0') {
                '_'
            } else {
                c
            }
        })
        .collect()
}

/// Expands `%[filename]`, `%[page]`, `%[bookmark]`, `%[index]`, `%[date]` in
/// `pattern` into a single file-name component (not a path).
///
/// `/`, `\`, `:` and NUL are replaced with `_` in every substituted value and in
/// the result; a result of "", "." or ".." becomes "untitled". A `None` bookmark
/// expands to "". Unknown `%[..]` keys are kept literally. No I/O.
pub fn expand(pattern: &str, ctx: &NameCtx) -> PathBuf {
    let mut out = String::new();
    let mut rest = pattern;
    while let Some(start) = rest.find("%[") {
        out.push_str(&rest[..start]);
        let tail = &rest[start + 2..];
        let Some(end) = tail.find(']') else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let value = match &tail[..end] {
            "filename" => Some(ctx.filename.clone()),
            "page" => Some(ctx.page.to_string()),
            "bookmark" => Some(ctx.bookmark.clone().unwrap_or_default()),
            "index" => Some(ctx.index.to_string()),
            "date" => Some(ctx.date.clone()),
            _ => None,
        };
        match value {
            Some(v) => out.push_str(&sanitize(&v)),
            None => out.push_str(&rest[start..start + 3 + end]),
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    let out = sanitize(&out);
    if matches!(out.as_str(), "" | "." | "..") {
        return PathBuf::from("untitled");
    }
    PathBuf::from(out)
}

/// True if anything, even a dangling symlink, occupies `p`.
fn taken(p: &Path) -> bool {
    p.symlink_metadata().is_ok()
}

/// Returns `path` if nothing exists there, else `stem-1.ext`, `stem-2.ext`, ...
/// in the same directory (`name-1` when there is no extension).
///
/// Best-effort only: the check races with other writers (TOCTOU), so callers must
/// create the file with `OpenOptions::create_new(true)` and retry on `AlreadyExists`.
pub fn unique_path(path: PathBuf) -> PathBuf {
    if !taken(&path) {
        return path;
    }
    let stem = crate::output::stem(&path);
    let ext = path.extension().map(|e| e.to_string_lossy().into_owned());
    let dir = path.parent().unwrap_or_else(|| Path::new(""));
    (1u64..)
        .map(|n| match &ext {
            Some(e) => dir.join(format!("{stem}-{n}.{e}")),
            None => dir.join(format!("{stem}-{n}")),
        })
        .find(|p| !taken(p))
        .unwrap_or(path)
}

/// macOS/Darwin `struct tm`.
#[repr(C)]
#[derive(Default)]
struct Tm {
    tm_sec: i32,
    tm_min: i32,
    tm_hour: i32,
    tm_mday: i32,
    tm_mon: i32,
    tm_year: i32,
    tm_wday: i32,
    tm_yday: i32,
    tm_isdst: i32,
    tm_gmtoff: i64,
    tm_zone: usize, // `char *`, never read
}

unsafe extern "C" {
    fn localtime_r(t: *const i64, out: *mut Tm) -> *mut Tm;
}

fn fmt_tm(tm: &Tm) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday
    )
}

/// `YYYY-MM-DD` of unix time `t` in the local time zone; `None` if libc fails.
fn local_date(t: i64) -> Option<String> {
    let mut tm = Tm::default();
    // SAFETY: `t` and `tm` are valid for the call; `Tm` matches Darwin's
    // `struct tm`; `localtime_r` writes only into `tm` and returns null on failure.
    let r = unsafe { localtime_r(&t, &mut tm) };
    (!r.is_null()).then(|| fmt_tm(&tm))
}

/// Today's date as `YYYY-MM-DD` in the local time zone (libc `localtime_r`).
/// If libc cannot convert the time, falls back to the UTC date.
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    local_date(secs).unwrap_or_else(|| civil_date(secs / 86_400))
}

/// Days since 1970-01-01 to `YYYY-MM-DD` (Howard Hinnant's civil-from-days).
pub(crate) fn civil_date(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod date_tests {
    use super::*;

    #[test]
    fn civil_dates() {
        assert_eq!(civil_date(0), "1970-01-01");
        assert_eq!(civil_date(11_016), "2000-02-29");
        assert_eq!(civil_date(20_088), "2024-12-31");
        assert_eq!(today().len(), 10);
    }

    #[test]
    fn local_date_formats_tm_fields() {
        let tm = Tm {
            tm_year: 124,
            tm_mon: 11,
            tm_mday: 5,
            ..Tm::default()
        };
        assert_eq!(fmt_tm(&tm), "2024-12-05");
    }

    #[test]
    fn local_date_is_within_a_day_of_utc() {
        let t = 1_000_000_000; // 2001-09-09T01:46:40Z; offsets span -12h..+14h
        let got = local_date(t).unwrap();
        assert!(
            ["2001-09-08", "2001-09-09", "2001-09-10"].contains(&got.as_str()),
            "{got}"
        );
    }
}
