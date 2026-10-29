//! Input structs of the remaining ops, built from the JSON a client sends:
//! defaults (output beside the input, split pattern, bookmark level) and
//! input validation before any default directory is used.
mod common;
use common::{copy_fixture, names, setup};
use monax_core::pages::*;
use monax_core::pdfium::{ExtractImagesInput, page_count};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn input<T: DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v).unwrap()
}

fn file_names(v: &[PathBuf]) -> Vec<String> {
    let name = |p: &PathBuf| p.file_name().unwrap().to_string_lossy().into_owned();
    v.iter().map(name).collect()
}

#[test]
fn single_output_inputs_default_beside_the_input() {
    require_qpdf!();
    let (dir, p) = setup();
    let d = dir.path();
    let n = |o: &Path| page_count(o).unwrap();
    let rect = json!({ "left": 10, "bottom": 10, "right": 100, "top": 100 });
    let cases: [(PathBuf, &str, usize); 5] = [
        (
            input::<AlternateMixInput>(json!({ "a": p, "b": p, "reverse_b": true }))
                .run()
                .unwrap()
                .0,
            "three-alternate-mix.pdf",
            6,
        ),
        (
            input::<NupInput>(json!({ "input": p, "per_sheet": 2 }))
                .run()
                .unwrap()
                .0,
            "three-nup.pdf",
            2,
        ),
        (
            input::<CropInput>(json!({ "input": p, "pages": [0], "rect": rect }))
                .run()
                .unwrap()
                .0,
            "three-crop.pdf",
            3,
        ),
        (
            input::<InsertBlankInput>(json!({ "input": p, "after": [0, 2] }))
                .run()
                .unwrap()
                .0,
            "three-insert-blank.pdf",
            5,
        ),
        (
            input::<SplitHalfInput>(json!({ "input": p }))
                .run()
                .unwrap()
                .0,
            "three-split-half.pdf",
            6,
        ),
    ];
    for (out, name, pages) in cases {
        assert_eq!((out.clone(), n(&out)), (d.join(name), pages));
    }
}

#[test]
fn splits_default_to_the_input_dir_and_index_pattern() {
    require_qpdf!();
    let (dir, p) = setup();
    let ranges = json!([{ "start": 0, "end": 0 }, { "start": 1, "end": 2 }]);
    let got = input::<SplitPagesInput>(json!({ "input": p, "ranges": ranges }))
        .run()
        .unwrap()
        .0;
    assert_eq!(file_names(&got), ["three-1.pdf", "three-2.pdf"]);
    assert_eq!(got[0].parent().unwrap(), dir.path());
    let got = input::<SplitTextInput>(json!({ "input": p, "text": "two" }))
        .run()
        .unwrap()
        .0;
    assert_eq!(file_names(&got), ["three-1-1.pdf", "three-2-1.pdf"]);
    let got = input::<SplitSizeInput>(json!({ "input": p, "max_bytes": u32::MAX }))
        .run()
        .unwrap()
        .0;
    assert_eq!(page_count(&got[0]).unwrap(), 3);
    let (_d, o) = copy_fixture("outlines.pdf", "o.pdf");
    let i: SplitBookmarksInput = input(json!({ "input": o }));
    assert_eq!((i.level, i.pattern.as_deref()), (1, None));
    assert_eq!(file_names(&i.run().unwrap().0), ["o-1.pdf", "o-2.pdf"]);
}

#[test]
fn explicit_out_dir_and_pattern_are_honoured() {
    require_qpdf!();
    let (_d, p) = setup();
    let out = tempfile::tempdir().unwrap();
    let v = json!({ "input": p, "ranges": [{ "start": 2, "end": 2 }],
                    "out_dir": out.path(), "pattern": "p%[page].pdf" });
    let got = input::<SplitPagesInput>(v).run().unwrap().0;
    assert_eq!(got, [out.path().join("p3.pdf")]);
}

#[test]
fn extract_images_defaults_beside_the_input() {
    require_pdfium!();
    let (dir, p) = copy_fixture("image.pdf", "img.pdf");
    let got = input::<ExtractImagesInput>(json!({ "input": p }))
        .run()
        .unwrap()
        .0;
    assert_eq!(got, [dir.path().join("img-p1-1.png")]);
}

#[test]
fn bad_input_is_reported_before_the_default_dir_and_writes_nothing() {
    let (dir, _p) = setup();
    let before = names(dir.path());
    let e = input::<SplitHalfInput>(json!({ "input": "rel.pdf" })).run();
    assert_eq!(
        e.unwrap_err().to_string(),
        "invalid input: path must be absolute: rel.pdf"
    );
    let e = input::<SplitTextInput>(json!({ "input": "rel.pdf", "text": "a" })).run();
    assert_eq!(
        e.unwrap_err().to_string(),
        "invalid input: path must be absolute: rel.pdf"
    );
    let missing = dir.path().join("none.pdf");
    let e = input::<ExtractImagesInput>(json!({ "input": missing })).run();
    let want = format!("invalid input: file does not exist: {}", missing.display());
    assert_eq!(e.unwrap_err().to_string(), want);
    assert_eq!(names(dir.path()), before);
}

#[test]
fn unknown_fields_are_rejected() {
    let bad = [
        serde_json::from_value::<SplitPagesInput>(json!({ "input": "/a", "ranges": [], "x": 1 }))
            .is_err(),
        serde_json::from_value::<ExtractImagesInput>(json!({ "input": "/a", "out": "/b" }))
            .is_err(),
        serde_json::from_value::<CropInput>(json!({ "input": "/a", "pages": [0],
            "rect": { "left": 0, "bottom": 0, "right": 1, "top": 1, "z": 0 } }))
        .is_err(),
    ];
    assert_eq!(bad, [true; 3]);
}
