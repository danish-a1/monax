mod common;
use common::{copy_fixture, names, setup};
use monax_core::pages::PagesError;
use monax_core::pdfium;
use monax_core::read::{PageInfoInput, ReadTextInput, RenderPageInput};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn info(input: &Path) -> PageInfoInput {
    PageInfoInput {
        input: input.into(),
        out: None,
        password: None,
    }
}

fn text(input: &Path, pages: Option<Vec<usize>>) -> ReadTextInput {
    ReadTextInput {
        input: input.into(),
        pages,
        out: None,
        password: None,
    }
}

fn render(input: &Path, page: usize, max_px: u32) -> RenderPageInput {
    RenderPageInput {
        input: input.into(),
        page,
        max_px,
        out: None,
        password: None,
    }
}

fn png_dims(p: &Path) -> (u32, u32) {
    let png = std::fs::read(p).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let r = |o: usize| u32::from_be_bytes(png[o..o + 4].try_into().unwrap());
    (r(16), r(20))
}

#[test]
fn page_info_matches_the_geometry_queries() {
    require_pdfium!();
    let (dir, p) = copy_fixture("rotated-crop.pdf", "rc.pdf");
    let out = info(&p).run().unwrap().0;
    assert_eq!(out, dir.path().join("rc-page-info.json"));
    let v: Value = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
    let n = pdfium::page_count(&p).unwrap();
    assert_eq!(v["page_count"], n);
    for i in 0..n {
        let pg = &v["pages"][i];
        let (l, b, r, t) = pdfium::crop_box(&p, i).unwrap();
        assert_eq!(pg["crop_box"], json!([l, b, r, t]), "page {i}");
        assert_eq!(pg["rotation"], pdfium::rotation(&p, i).unwrap(), "page {i}");
        let m = pg["media_box"].as_array().unwrap();
        let f = |k: usize| m[k].as_f64().unwrap() as f32;
        assert_eq!(
            (f(2) - f(0), f(3) - f(1)),
            pdfium::page_size(&p, i).unwrap()
        );
    }
}

#[test]
fn page_info_prints_boxes_as_their_f32_values() {
    require_pdfium!();
    let (dir, p) = setup();
    let r = monax_core::pages::Rect {
        left: 10.1,
        bottom: 20.1,
        right: 110.1,
        top: 120.1,
    };
    let cropped = dir.path().join("cropped.pdf");
    monax_core::pages::crop(&p, &[0], r, &cropped).unwrap();
    let out = info(&cropped).run().unwrap().0;
    let json = std::fs::read_to_string(out).unwrap();
    assert!(
        json.contains(r#""crop_box":[10.1,20.1,110.1,120.1]"#),
        "{json}"
    );
}

#[test]
fn read_text_all_pages_and_a_selection() {
    require_pdfium!();
    let (dir, p) = setup();
    let out = text(&p, None).run().unwrap().0;
    assert_eq!(out, dir.path().join("three-read-text.txt"));
    let all = std::fs::read_to_string(&out).unwrap();
    let pages: Vec<&str> = all.split('\u{c}').collect();
    assert_eq!(pages.len(), 3, "{all:?}");
    assert_eq!(pages[0], pdfium::extract_text(&p, 0..=0).unwrap());
    let out = text(&p, Some(vec![2, 0])).run().unwrap().0;
    assert_eq!(out, dir.path().join("three-read-text-1.txt"));
    let sel = std::fs::read_to_string(&out).unwrap();
    assert_eq!(sel, format!("{}\u{c}{}", pages[2], pages[0]));
}

#[test]
fn render_page_bounds_the_longer_side() {
    require_pdfium!();
    let (dir, p) = setup();
    let out = render(&p, 1, 300).run().unwrap().0;
    assert_eq!(out, dir.path().join("three-render-page.png"));
    assert_eq!(png_dims(&out).0.max(png_dims(&out).1), 300);
    let dflt: RenderPageInput = serde_json::from_value(json!({ "input": p, "page": 0 })).unwrap();
    assert_eq!(dflt.max_px, 1024);
}

#[test]
fn bad_requests_fail_and_leave_nothing() {
    require_pdfium!();
    let (dir, p) = setup();
    let range = |r: Result<(PathBuf, Vec<String>), PagesError>| {
        assert!(
            matches!(r, Err(PagesError::PageOutOfRange { page: 3, count: 3 })),
            "{r:?}"
        );
    };
    range(text(&p, Some(vec![0, 3])).run());
    range(render(&p, 3, 64).run());
    let invalid = |r: Result<(PathBuf, Vec<String>), PagesError>| {
        assert!(matches!(r, Err(PagesError::InvalidInput(_))), "{r:?}");
    };
    invalid(text(&p, Some(vec![])).run());
    invalid(render(&p, 0, 2000).run());
    invalid(info(Path::new("rel.pdf")).run());
    let mut over = info(&p);
    over.out = Some(p.clone());
    invalid(over.run());
    assert_eq!(names(dir.path()), ["three.pdf"]);
}

#[test]
fn encrypted_and_damaged_inputs_are_reported() {
    require_pdfium!();
    let (dir, enc) = copy_fixture("encrypted.pdf", "enc.pdf");
    let bad = dir.path().join("bad.pdf");
    std::fs::write(&bad, b"%PDF-1.7\ntruncated").unwrap();
    for r in [
        info(&enc).run(),
        text(&enc, None).run(),
        render(&enc, 0, 64).run(),
    ] {
        assert!(matches!(r, Err(PagesError::EncryptedInput)), "{r:?}");
    }
    for r in [
        info(&bad).run(),
        text(&bad, None).run(),
        render(&bad, 0, 64).run(),
    ] {
        assert!(matches!(r, Err(PagesError::Damaged)), "{r:?}");
    }
    assert_eq!(names(dir.path()), ["bad.pdf", "enc.pdf"]);
}

#[test]
fn inputs_reject_unknown_fields() {
    let e = serde_json::from_value::<ReadTextInput>(json!({ "input": "/a.pdf", "x": 1 }));
    assert!(e.unwrap_err().to_string().contains("unknown field"));
}
