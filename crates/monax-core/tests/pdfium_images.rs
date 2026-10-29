mod common;
use common::fixture;
use monax_core::pdfium::{self, PdfiumError};

#[test]
fn extracts_png_and_never_overwrites() {
    require_pdfium!();
    let dir = tempfile::tempdir().unwrap();
    let first = pdfium::extract_images(&fixture("image.pdf"), dir.path()).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].file_name().unwrap(), "image-p1-1.png");
    let dec = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(&first[0]).unwrap(),
    ));
    let reader = dec.read_info().unwrap();
    assert_eq!((reader.info().width, reader.info().height), (2, 2));
    let before = std::fs::read(&first[0]).unwrap();

    let second = pdfium::extract_images(&fixture("image.pdf"), dir.path()).unwrap();
    assert_eq!(second.len(), 1);
    assert_ne!(second[0], first[0]);
    assert_eq!(second[0].file_name().unwrap(), "image-p1-1-1.png");
    assert_eq!(std::fs::read(&first[0]).unwrap(), before);
}

#[test]
fn no_images_writes_nothing() {
    require_pdfium!();
    let dir = tempfile::tempdir().unwrap();
    let out = pdfium::extract_images(&fixture("three-pages.pdf"), dir.path()).unwrap();
    assert!(out.is_empty());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn out_dir_must_be_existing_directory() {
    require_pdfium!();
    let dir = tempfile::tempdir().unwrap();
    let src = fixture("image.pdf");
    let missing = dir.path().join("missing");
    assert!(matches!(
        pdfium::extract_images(&src, &missing),
        Err(PdfiumError::InvalidInput(_))
    ));
    assert!(matches!(
        pdfium::extract_images(&src, &src),
        Err(PdfiumError::InvalidInput(_))
    ));
}
