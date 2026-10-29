mod common;
use monax_core::content::*;
use serde_json::json;
use std::path::Path;

#[test]
fn opts_default_and_parse_snake_case_anchors() {
    let i: PageNumbersInput = serde_json::from_value(json!({"input": "/a.pdf"})).unwrap();
    assert_eq!(i.opts, NumberOpts::default());
    assert_eq!(i.opts.format, "%[page]");
    let v = json!({"input": "/a.pdf", "opts": {"position": "top_right", "pages": [1]}});
    let i: BatesInput = serde_json::from_value(v).unwrap();
    assert_eq!(
        (i.opts.position, i.opts.pages),
        (Anchor::TopRight, Some(vec![1]))
    );
    assert_eq!(i.opts.digits, 6);
    let v = json!({"input": "/a.pdf", "opts": {"header": {"center": "T"}}});
    let i: HeaderFooterInput = serde_json::from_value(v).unwrap();
    assert_eq!(i.opts.header.center, "T");
    let v = json!({"input": "/a.pdf", "opts": {"text": "W", "color": [1, 2, 3]}});
    let i: WatermarkInput = serde_json::from_value(v).unwrap();
    assert_eq!((i.opts.color, i.opts.opacity), ([1, 2, 3], 0.3));
}

#[test]
fn unknown_fields_and_anchors_are_rejected() {
    let bad = [
        json!({"input": "/a.pdf", "opts": {"bogus": 1}}),
        json!({"input": "/a.pdf", "opts": {"position": "middle"}}),
        json!({"input": "/a.pdf", "bogus": 1}),
    ];
    for v in bad {
        assert!(serde_json::from_value::<PageNumbersInput>(v).is_err());
    }
    let v = json!({"input": "/a.pdf", "opts": {"color": [256, 0, 0]}});
    assert!(serde_json::from_value::<WatermarkInput>(v).is_err());
    // R71: no underlay mode.
    let v = json!({"input": "/a.pdf", "opts": {"text": "W", "behind": true}});
    assert!(serde_json::from_value::<WatermarkInput>(v).is_err());
}

#[test]
fn each_input_runs_into_its_default_output() {
    require_pdfium!();
    let (dir, input) = common::setup();
    let at = |n: &str| dir.path().join(n);
    let w = WatermarkOpts {
        text: "W".into(),
        ..WatermarkOpts::default()
    };
    let hf = HfOpts {
        header: Row {
            left: "H".into(),
            ..Row::default()
        },
        ..HfOpts::default()
    };
    let runs = [
        (
            PageNumbersInput {
                input: input.clone(),
                opts: NumberOpts::default(),
                out: None,
                password: None,
            }
            .run(),
            "three-page-numbers.pdf",
        ),
        (
            HeaderFooterInput {
                input: input.clone(),
                opts: hf,
                out: None,
                password: None,
            }
            .run(),
            "three-header-footer.pdf",
        ),
        (
            WatermarkInput {
                input: input.clone(),
                opts: w,
                out: None,
                password: None,
            }
            .run(),
            "three-watermark.pdf",
        ),
        (
            BatesInput {
                input: input.clone(),
                opts: BatesOpts::default(),
                out: Some(at("b.pdf")),
                password: None,
            }
            .run(),
            "b.pdf",
        ),
    ];
    for (r, name) in runs {
        assert_eq!(r.unwrap().0, at(name));
    }
    let r = PageNumbersInput {
        input: Path::new("rel.pdf").into(),
        opts: NumberOpts::default(),
        out: None,
        password: None,
    }
    .run();
    assert!(matches!(r, Err(PagesError::InvalidInput(_))));
}
