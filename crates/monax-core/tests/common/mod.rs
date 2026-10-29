#![allow(dead_code)] // each test binary uses a subset of these helpers
pub mod annot;
pub mod convert;
pub mod docfeat;
pub mod docs;
pub mod forms;
pub mod guard;
pub mod leaky;
pub mod ocr;
pub mod office;
pub mod probe;
pub mod probe10;
pub mod probe11;
pub mod probe12;
pub mod probe13;
pub mod probe14;
pub mod probe15;
pub mod probe16;
pub mod probe17;
pub mod probe3;
pub mod probe4;
pub mod probe5;
pub mod probe6;
pub mod probe7;
pub mod probe8;

pub mod scan;
pub mod secret;
pub mod sign;

use monax_core::pdfium;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// Copies fixture `name` into a fresh temp dir as `as_name`.
pub fn copy_fixture(name: &str, as_name: &str) -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join(as_name);
    std::fs::copy(fixture(name), &p).unwrap();
    (dir, p)
}

/// `three-pages.pdf` copied into a fresh temp dir as `three.pdf`.
pub fn setup() -> (TempDir, PathBuf) {
    copy_fixture("three-pages.pdf", "three.pdf")
}

/// Sorted entry names of `dir`.
pub fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

/// Last word of each page's text ("Page two" -> "two"); "" for a blank page.
pub fn words(p: &Path) -> Vec<String> {
    let n = pdfium::page_count(p).unwrap();
    (0..n)
        .map(|i| {
            let t = pdfium::extract_text(p, i..=i).unwrap();
            t.split_whitespace().last().unwrap_or("").to_string()
        })
        .collect()
}

/// /Rotate of every page.
pub fn rotations(p: &Path) -> Vec<u16> {
    let n = pdfium::page_count(p).unwrap();
    (0..n).map(|i| pdfium::rotation(p, i).unwrap()).collect()
}

/// Returns `ok`. When false: panics if `strict_var` or `CI` is set (strict
/// mode); otherwise writes a skip notice straight to stderr (bypasses libtest
/// capture, so it is always visible).
fn available(ok: bool, why: &str, strict_var: &str) -> bool {
    if !ok {
        let msg = format!("SKIPPED: {why} (strict mode: {strict_var}=1 or CI)\n");
        assert!(
            std::env::var_os(strict_var).is_none() && std::env::var_os("CI").is_none(),
            "{msg}"
        );
        let _ = std::io::Write::write_all(&mut std::io::stderr(), msg.as_bytes());
    }
    ok
}

/// True when the pdfium dylib is available (strict: `MONAX_REQUIRE_PDFIUM`).
pub fn have_pdfium() -> bool {
    let ok =
        std::env::var_os("MONAX_LIBPDFIUM").is_some_and(|p| std::path::Path::new(&p).is_file());
    available(
        ok,
        "MONAX_LIBPDFIUM not set or library missing",
        "MONAX_REQUIRE_PDFIUM",
    )
}

/// True when qpdf is found (strict: `MONAX_REQUIRE_SIDECARS`).
pub fn have_qpdf() -> bool {
    let ok = monax_sidecar::find(monax_sidecar::Tool::Qpdf).is_some();
    available(ok, "qpdf not found", "MONAX_REQUIRE_SIDECARS")
}

/// Early-returns from the test (with a notice) when pdfium is unavailable.
#[macro_export]
macro_rules! require_pdfium {
    () => {
        if !common::have_pdfium() {
            return;
        }
    };
}

/// `<$t>::run()` on the input struct deserialized from JSON `$v`.
#[macro_export]
macro_rules! run_input {
    ($t:ty, $v:expr) => {
        serde_json::from_value::<$t>($v).unwrap().run()
    };
}

/// Early-returns from the test (with a notice) when pdfium or qpdf is missing.
#[macro_export]
macro_rules! require_qpdf {
    () => {
        if !common::have_pdfium() || !common::have_qpdf() {
            return;
        }
    };
}

/// Pixel box `(x0, y0, x1, y1)` (inclusive, top-left origin) of every pixel
/// whose color differs by more than 16 between 72 dpi renders of `page` in
/// `a` and `b` (same displayed size); `None` when they match.
pub fn diff_box(a: &Path, b: &Path, page: usize) -> Option<(u32, u32, u32, u32)> {
    diff_box_in(a, b, page, 0..u32::MAX, 0..u32::MAX)
}

/// [`diff_box`] limited to pixel columns `cols` and rows `rows`.
pub fn diff_box_in(
    a: &Path,
    b: &Path,
    page: usize,
    cols: std::ops::Range<u32>,
    rows: std::ops::Range<u32>,
) -> Option<(u32, u32, u32, u32)> {
    let (ra, rb) = (
        pdfium::render(a, page, 72).unwrap(),
        pdfium::render(b, page, 72).unwrap(),
    );
    assert_eq!((ra.w, ra.h), (rb.w, rb.h), "page sizes differ");
    let mut bx: Option<(u32, u32, u32, u32)> = None;
    for (i, (pa, pb)) in ra.rgba.chunks(4).zip(rb.rgba.chunks(4)).enumerate() {
        if pa.iter().zip(pb).all(|(x, y)| x.abs_diff(*y) <= 16) {
            continue;
        }
        let (x, y) = (i as u32 % ra.w, i as u32 / ra.w);
        if !cols.contains(&x) || !rows.contains(&y) {
            continue;
        }
        bx = Some(bx.map_or((x, y, x, y), |(x0, y0, x1, y1)| {
            (x0.min(x), y0.min(y), x1.max(x), y1.max(y))
        }));
    }
    bx
}
