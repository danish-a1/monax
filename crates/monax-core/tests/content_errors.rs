//! Content ops: guard cases, option errors, character set.
mod common;
use common::guard::{Op, invalid};
use common::{names, setup};
use monax_core::content::{self, BatesOpts, HfOpts, NumberOpts, Row, WatermarkOpts};
use monax_core::pages::PagesError;
use monax_core::pdfium;

const OPS: [(&str, Op); 4] = [
    ("page_numbers", |i, o| {
        content::page_numbers(i, o, NumberOpts::default())
    }),
    ("header_footer", |i, o| {
        content::header_footer(i, o, hf("H", "F"))
    }),
    ("watermark", |i, o| content::watermark(i, o, mark("DRAFT"))),
    ("bates", |i, o| content::bates(i, o, BatesOpts::default())),
];

guard_tests!(OPS, common::have_pdfium());

fn hf(header_left: &str, footer_right: &str) -> HfOpts {
    HfOpts {
        header: Row {
            left: header_left.into(),
            ..Row::default()
        },
        footer: Row {
            right: footer_right.into(),
            ..Row::default()
        },
        margin: 10.0,
        ..HfOpts::default()
    }
}

fn mark(text: &str) -> WatermarkOpts {
    WatermarkOpts {
        text: text.into(),
        ..WatermarkOpts::default()
    }
}

#[test]
fn bad_options_fail_without_output() {
    require_pdfium!();
    let (dir, three) = setup();
    let out = dir.path().join("o.pdf");
    let wm = |o: WatermarkOpts| content::watermark(&three, &out, o);
    let msg = |r: Result<_, PagesError>| r.unwrap_err().to_string();
    assert!(msg(wm(mark(""))).contains("overlay text is empty"));
    assert!(
        msg(wm(WatermarkOpts {
            opacity: -0.1,
            ..mark("x")
        }))
        .contains("opacity")
    );
    assert!(
        msg(wm(WatermarkOpts {
            angle: f32::NAN,
            ..mark("x")
        }))
        .contains("angle")
    );
    assert!(
        msg(wm(WatermarkOpts {
            size: 0.0,
            ..mark("x")
        }))
        .contains("size")
    );
    assert_eq!(
        msg(wm(mark("caf\u{e9} \u{3b1}"))),
        "invalid input: character '\u{3b1}' (U+03B1) is not supported by Helvetica (WinAnsi)"
    );
    assert_eq!(
        msg(wm(WatermarkOpts {
            size: 500.0,
            ..mark("HUGE")
        })),
        "invalid input: overlay text does not fit on page 0 at size 500"
    );
    let n = |pages| {
        content::page_numbers(
            &three,
            &out,
            NumberOpts {
                pages,
                ..NumberOpts::default()
            },
        )
    };
    assert!(invalid(n(Some(vec![]))));
    assert!(matches!(
        n(Some(vec![1, 3])),
        Err(PagesError::PageOutOfRange { page: 3, count: 3 })
    ));
    assert_eq!(names(dir.path()), ["three.pdf"]);
}

#[test]
fn winansi_extras_round_trip() {
    require_pdfium!();
    let (dir, three) = setup();
    let s = "\u{20ac}5 \u{2014} caf\u{e9} \u{2122}";
    let out = content::watermark(
        &three,
        &dir.path().join("w.pdf"),
        WatermarkOpts {
            size: 12.0,
            ..mark(s)
        },
    )
    .unwrap();
    let t = pdfium::extract_text(&out, 0..=0).unwrap();
    assert!(t.contains(s), "{t}");
}
