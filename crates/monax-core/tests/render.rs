mod common;
use common::fixture;
use monax_core::pdfium;
use std::sync::Arc;

#[test]
fn renders_and_extracts() {
    require_pdfium!();
    let p = fixture("three-pages.pdf");
    pdfium::init().unwrap();
    assert_eq!(pdfium::page_count(&p).unwrap(), 3);
    let r = pdfium::render(&p, 0, 72).unwrap();
    assert!(r.w > 100 && r.h > 100);
    assert_eq!(r.rgba.len(), r.w as usize * r.h as usize * 4);
    assert!(r.rgba.iter().any(|&b| b != 0));
    assert!(
        pdfium::extract_text(&p, 0..=0)
            .unwrap()
            .contains("Page one")
    );
    let all = pdfium::extract_text(&p, 0..=2).unwrap();
    assert!(all.contains("Page two") && all.contains("Page three"));
}

#[test]
fn concurrent_render_is_serialized() {
    require_pdfium!();
    let p = Arc::new(fixture("three-pages.pdf"));
    let handles: Vec<_> = (0..8)
        .map(|t| {
            let p = Arc::clone(&p);
            std::thread::spawn(move || {
                for i in 0..4 {
                    let r = pdfium::render(&p, (t + i) % 3, 72).unwrap();
                    assert!(r.w > 0 && r.h > 0);
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
}

#[test]
fn page_size_and_rotation_plain() {
    require_pdfium!();
    let p = fixture("three-pages.pdf");
    assert_eq!(pdfium::page_size(&p, 0).unwrap(), (200.0, 200.0));
    assert_eq!(pdfium::rotation(&p, 0).unwrap(), 0);
}

#[test]
fn rotation_and_cropbox_reflected() {
    require_pdfium!();
    let p = fixture("rotated-crop.pdf");
    assert_eq!(pdfium::page_size(&p, 0).unwrap(), (300.0, 200.0));
    assert_eq!(pdfium::rotation(&p, 0).unwrap(), 90);
    // CropBox 200x100 rotated 90 degrees => 100 wide, 200 tall.
    let r = pdfium::render(&p, 0, 72).unwrap();
    assert_eq!((r.w, r.h), (100, 200));
    let r2 = pdfium::render(&p, 0, 144).unwrap();
    assert_eq!((r2.w, r2.h), (200, 400));
}

#[test]
fn extracts_all_text_on_a_rotated_cropped_page() {
    require_pdfium!();
    // /Rotate 90, CropBox [50 20 250 120]: text must not be clipped by the
    // displayed size taken as a user space rectangle.
    let t = pdfium::extract_text(&fixture("rotated-crop.pdf"), 0..=0).unwrap();
    assert_eq!(t, "Rotated");
}
