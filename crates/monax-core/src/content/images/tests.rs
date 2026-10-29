//! Unit tests of image selection: color spaces, plans, target sizes.
use super::*;

fn doc() -> Doc {
    Doc::of(json!({
        "obj:1 0 R": { "value": ["/ICCBased", "2 0 R"] },
        "obj:2 0 R": { "stream": { "dict": { "/N": 3 } } },
        "obj:3 0 R": { "value": ["/ICCBased", "4 0 R"] },
        "obj:4 0 R": { "stream": { "dict": { "/N": 4 } } },
    }))
}

#[test]
fn channels_of_gray_rgb_cal_and_icc_spaces() {
    let d = doc();
    let n = |cs: Value| channels(&d, &cs);
    assert_eq!(n(json!("/DeviceGray")), Some(1));
    assert_eq!(n(json!("/DeviceRGB")), Some(3));
    assert_eq!(n(json!(["/CalRGB", {}])), Some(3));
    assert_eq!(n(json!(["/CalGray", {}])), Some(1));
    assert_eq!(n(json!("1 0 R")), Some(3));
    assert_eq!(n(json!("3 0 R")), None); // CMYK ICC
    assert_eq!(n(json!("/DeviceCMYK")), None);
    assert_eq!(n(json!(["/Indexed", "/DeviceRGB", 1, "b:00"])), None);
    assert_eq!(n(json!("9 0 R")), None); // dangling
}

#[test]
fn plan_skips_masked_odd_depth_and_undrawn_images() {
    let d = Doc::of(json!({
        "obj:1 0 R": { "value": ["/ICCBased", "2 0 R"] },
        "obj:2 0 R": { "stream": { "dict": { "/N": 3 } } },
        "obj:5 0 R": { "stream": { "dict": { "/Matte": [0, 0, 0] } } },
        "obj:6 0 R": { "stream": { "dict": { "/Width": 10 } } },
    }));
    let ext: Extents = [((600, 600), (144.0, 144.0))].into();
    let img = |extra: Value| {
        let mut m = json!({ "/Width": 600, "/Height": 600, "/BitsPerComponent": 8,
            "/ColorSpace": "/DeviceRGB" });
        m.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        plan(&d, m.as_object().unwrap(), &ext, 72).map(|p| p.map(|p| (p.to, p.n)))
    };
    assert_eq!(img(json!({})), Ok(Some(((144, 144), 3))));
    assert_eq!(img(json!({ "/SMask": "6 0 R" })), Ok(Some(((144, 144), 3))));
    assert_eq!(img(json!({ "/SMask": "5 0 R" })), Err(Skip::Masked)); // /Matte
    assert_eq!(img(json!({ "/ImageMask": true })), Err(Skip::BitDepth));
    assert_eq!(img(json!({ "/Mask": [0, 0] })), Err(Skip::Masked));
    assert_eq!(img(json!({ "/Decode": [1, 0] })), Err(Skip::Decode));
    assert_eq!(img(json!({ "/BitsPerComponent": 1 })), Err(Skip::BitDepth));
    assert_eq!(img(json!({ "/Width": 300 })), Err(Skip::NotDrawn));
    assert_eq!(
        img(json!({ "/ColorSpace": "/DeviceCMYK" })),
        Err(Skip::ColorSpace)
    );
    assert_eq!(
        img(json!({ "/ColorSpace": "1 0 R" })),
        Ok(Some(((144, 144), 3)))
    );
    let fine = plan(
        &d,
        json!({ "/Width": 600, "/Height": 600 })
            .as_object()
            .unwrap(),
        &ext,
        300,
    );
    assert!(matches!(fine, Ok(None)));
    assert!(matches!(plan(&d, &Map::new(), &ext, 72), Ok(None)));
}

#[test]
fn notes_count_each_reason_in_order() {
    let skipped = [
        (Skip::NotDrawn, 1),
        (Skip::ColorSpace, 3),
        (Skip::NotSmaller, 1),
    ]
    .into();
    assert_eq!(
        notes(&skipped, 72),
        [
            "3 images left above 72 dpi: color space not gray or RGB",
            "1 image left above 72 dpi: a resampled JPEG would not be smaller",
            "1 image not checked against 72 dpi: only drawn outside page content \
             (patterns, annotations) or unused",
        ]
    );
}

#[test]
fn target_scales_to_dpi_and_never_upsamples() {
    // 600 px over 144 pt (2 in) is 300 dpi.
    assert_eq!(target((600, 600), (144.0, 144.0), 72), (144, 144));
    assert_eq!(target((600, 300), (144.0, 144.0), 150), (300, 300));
    assert_eq!(target((100, 100), (144.0, 144.0), 300), (100, 100));
    assert_eq!(target((100, 100), (0.0, 0.0), 72), (1, 1));
}
