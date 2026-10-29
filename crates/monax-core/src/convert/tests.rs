//! Unit tests of the conversion options (no pdfium, no qpdf).
use super::{ImageFormat, ImagesToPdfInput, PageSize, PdfToImagesInput};
use serde_json::json;

/// The sink of a finished job of its own.
fn sink() -> crate::ProgressSink {
    let (tx, rx) = std::sync::mpsc::channel();
    crate::Registry::new().submit(move |s| tx.send(s).map(|()| vec![]));
    rx.recv().unwrap()
}

#[test]
fn page_size_sides_are_72_to_14400_points() {
    let size = |width, height| PageSize { width, height };
    assert!(size(72.0, 14400.0).check().is_ok());
    let msg = |s: PageSize| s.check().unwrap_err().to_string();
    assert_eq!(
        msg(size(71.9, 100.0)),
        "invalid input: page size 71.9 pt not in 72..=14400"
    );
    assert_eq!(
        msg(size(100.0, f32::INFINITY)),
        "invalid input: page size inf pt not in 72..=14400"
    );
}

#[test]
fn pdf_to_images_defaults_to_150_dpi_png_quality_85_every_page() {
    let i: PdfToImagesInput = serde_json::from_value(json!({ "input": "/a.pdf" })).unwrap();
    assert_eq!(
        (i.dpi, i.format, i.jpeg_quality),
        (150, ImageFormat::Png, 85)
    );
    assert!(i.pages.is_none() && i.out_dir.is_none() && i.password.is_none());
    let j: PdfToImagesInput =
        serde_json::from_value(json!({ "input": "/a.pdf", "format": "jpeg" })).unwrap();
    assert_eq!(j.format, ImageFormat::Jpeg);
    for bad in [
        json!({ "input": "/a.pdf", "x": 1 }),
        json!({ "input": "/a.pdf", "format": "gif" }),
    ] {
        assert!(serde_json::from_value::<PdfToImagesInput>(bad).is_err());
    }
}

#[test]
fn images_to_pdf_input_needs_an_image_and_known_fields() {
    let i: ImagesToPdfInput = serde_json::from_value(json!({ "inputs": [] })).unwrap();
    assert_eq!(
        i.run(&sink()).unwrap_err().to_string(),
        "invalid input: no images given"
    );
    let v = json!({ "inputs": ["/a.png"], "page_size": { "width": 612, "height": 792 } });
    let i: ImagesToPdfInput = serde_json::from_value(v).unwrap();
    assert_eq!(
        i.page_size,
        Some(PageSize {
            width: 612.0,
            height: 792.0
        })
    );
    let extra = json!({ "inputs": ["/a.png"], "page_size": { "width": 1, "height": 1, "d": 1 } });
    assert!(serde_json::from_value::<ImagesToPdfInput>(extra).is_err());
}
