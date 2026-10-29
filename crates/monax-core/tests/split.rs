mod common;
use common::fixture;
use monax_core::pages::{self, PagesError, RangeSpec};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Last word of each page's text, per output file.
fn words(files: &[PathBuf]) -> Vec<Vec<String>> {
    files.iter().map(|f| common::words(f)).collect()
}

fn names(files: &[PathBuf]) -> Vec<String> {
    files
        .iter()
        .map(|f| f.file_name().unwrap().to_string_lossy().into_owned())
        .collect()
}

fn setup(fx: &str) -> (TempDir, PathBuf, TempDir) {
    let (dir, input) = common::copy_fixture(fx, "doc.pdf");
    (dir, input, tempfile::tempdir().unwrap())
}

fn r(start: usize, end: usize) -> RangeSpec {
    RangeSpec { start, end }
}

#[test]
fn split_pages_chunks_and_names() {
    require_qpdf!();
    let (_d, input, out) = setup("three-pages.pdf");
    let got = pages::split_pages(
        &input,
        &[r(0, 1), r(2, 2)],
        out.path(),
        "%[filename]-%[index]-p%[page].pdf",
    )
    .unwrap();
    assert_eq!(names(&got), ["doc-1-p1.pdf", "doc-2-p3.pdf"]);
    assert_eq!(words(&got), [vec!["one", "two"], vec!["three"]]);
}

#[test]
fn never_overwrites_existing_output() {
    require_qpdf!();
    let (_d, input, out) = setup("three-pages.pdf");
    std::fs::write(out.path().join("a.pdf"), "keep").unwrap();
    let got = pages::split_pages(&input, &[r(0, 0)], out.path(), "a.pdf").unwrap();
    assert_eq!(names(&got), ["a-1.pdf"]);
    assert_eq!(std::fs::read(out.path().join("a.pdf")).unwrap(), b"keep");
}

#[test]
fn date_placeholder_expands() {
    require_qpdf!();
    let (_d, input, out) = setup("three-pages.pdf");
    let got = pages::split_pages(&input, &[r(0, 0)], out.path(), "%[date].pdf").unwrap();
    let n = &names(&got)[0];
    assert!(n.len() == 14 && n.as_bytes()[4] == b'-', "{n}");
}

fn size_of(input: &Path, pages_: &[usize], dir: &Path) -> u64 {
    let p = pages::extract(input, pages_, &dir.join("probe.pdf")).unwrap();
    std::fs::metadata(p).unwrap().len()
}

#[test]
fn split_size_greedy_and_oversize() {
    require_qpdf!();
    let (d, input, out) = setup("three-pages.pdf");
    let two = size_of(&input, &[0, 1], d.path());
    let got = pages::split_size(&input, two, out.path(), "s-%[index].pdf").unwrap();
    assert_eq!(words(&got), [vec!["one", "two"], vec!["three"]]);
    for f in &got {
        assert!(std::fs::metadata(f).unwrap().len() <= two);
    }
    // Every page exceeds 1 byte: each is its own chunk, not an error.
    let got = pages::split_size(&input, 1, out.path(), "t-%[index].pdf").unwrap();
    assert_eq!(words(&got), [vec!["one"], vec!["two"], vec!["three"]]);
}

#[test]
fn split_text_boundaries() {
    require_qpdf!();
    let (_d, input, out) = setup("three-pages.pdf");
    let got = pages::split_text(&input, "two", out.path(), "a-%[index].pdf").unwrap();
    assert_eq!(words(&got), [vec!["one"], vec!["two", "three"]]);
    let got = pages::split_text(&input, "Page", out.path(), "b-%[index].pdf").unwrap();
    assert_eq!(got.len(), 3);
    let got = pages::split_text(&input, "nomatch", out.path(), "c-%[index].pdf").unwrap();
    assert_eq!(words(&got), [vec!["one", "two", "three"]]);
    let e = pages::split_text(&input, "", out.path(), "d.pdf");
    assert!(matches!(e, Err(PagesError::InvalidInput(_))), "{e:?}");
}

#[test]
fn split_bookmarks_levels_and_names() {
    require_qpdf!();
    let (_d, input, out) = setup("outlines.pdf");
    let got = pages::split_bookmarks(&input, 1, out.path(), "%[index]-%[bookmark].pdf").unwrap();
    assert_eq!(names(&got), ["1-Intro.pdf", "2-End.pdf"]);
    assert_eq!(words(&got), [vec!["one", "two"], vec!["three"]]);
    let got = pages::split_bookmarks(&input, 2, out.path(), "L2-%[bookmark].pdf").unwrap();
    assert_eq!(names(&got), ["L2-.pdf", "L2-Details.pdf"]); // leading pages, no title
    assert_eq!(words(&got), [vec!["one"], vec!["two", "three"]]);
    let e = pages::split_bookmarks(&input, 3, out.path(), "x.pdf");
    assert!(matches!(e, Err(PagesError::InvalidInput(_))), "{e:?}");
    let (_d, plain, out) = setup("three-pages.pdf");
    let e = pages::split_bookmarks(&plain, 1, out.path(), "x.pdf");
    assert!(matches!(e, Err(PagesError::InvalidInput(_))), "{e:?}");
    assert_eq!(std::fs::read_dir(out.path()).unwrap().count(), 0);
}

#[test]
fn bad_arguments_and_inputs() {
    require_qpdf!();
    let (d, input, out) = setup("three-pages.pdf");
    let bad = |x: Result<Vec<PathBuf>, PagesError>| x.unwrap_err();
    let o = out.path();
    assert!(matches!(
        bad(pages::split_pages(&input, &[r(2, 1)], o, "x.pdf")),
        PagesError::InvalidInput(_)
    ));
    assert!(matches!(
        bad(pages::split_pages(&input, &[], o, "x.pdf")),
        PagesError::InvalidInput(_)
    ));
    assert!(matches!(
        bad(pages::split_pages(&input, &[r(0, 3)], o, "x.pdf")),
        PagesError::PageOutOfRange { page: 3, count: 3 }
    ));
    assert!(matches!(
        bad(pages::split_size(&input, 0, o, "x.pdf")),
        PagesError::InvalidInput(_)
    ));
    for dir in [Path::new("rel"), &d.path().join("missing"), &input] {
        assert!(matches!(
            bad(pages::split_pages(&input, &[r(0, 0)], dir, "x.pdf")),
            PagesError::InvalidInput(_)
        ));
    }
    let enc = d.path().join("enc.pdf");
    std::fs::copy(fixture("encrypted.pdf"), &enc).unwrap();
    assert!(matches!(
        bad(pages::split_text(&enc, "a", o, "x.pdf")),
        PagesError::EncryptedInput
    ));
    let junk = d.path().join("junk.pdf");
    std::fs::write(&junk, "not a pdf").unwrap();
    assert!(matches!(
        bad(pages::split_bookmarks(&junk, 1, o, "x.pdf")),
        PagesError::Damaged
    ));
    assert_eq!(std::fs::read_dir(o).unwrap().count(), 0);
}
