mod common;
use monax_core::pages::*;
use monax_core::pdfium::page_count;
use std::path::Path;

fn src(dir: &Path) -> std::path::PathBuf {
    let p = dir.join("in.pdf");
    std::fs::copy(common::fixture("three-pages.pdf"), &p).unwrap();
    p
}

#[test]
fn each_input_runs_into_its_default_output() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let input = src(dir.path());
    let n = |p: &Path| page_count(p).unwrap();
    let out = ExtractInput {
        input: input.clone(),
        pages: vec![0, 1],
        out: None,
        password: None,
    }
    .run()
    .unwrap()
    .0;
    assert_eq!(
        (out.clone(), n(&out)),
        (dir.path().join("in-extract.pdf"), 2)
    );
    let out = DeleteInput {
        input: input.clone(),
        pages: vec![0],
        out: None,
        password: None,
    }
    .run()
    .unwrap()
    .0;
    assert_eq!(
        (out.clone(), n(&out)),
        (dir.path().join("in-delete.pdf"), 2)
    );
    let out = ReorderInput {
        input: input.clone(),
        order: vec![2, 1, 0],
        out: None,
        password: None,
    }
    .run()
    .unwrap()
    .0;
    assert_eq!(
        (out.clone(), n(&out)),
        (dir.path().join("in-reorder.pdf"), 3)
    );
    let out = RotateInput {
        input: input.clone(),
        pages: vec![0],
        degrees: 90,
        out: None,
        password: None,
    }
    .run()
    .unwrap()
    .0;
    assert_eq!(
        (out.clone(), n(&out)),
        (dir.path().join("in-rotate.pdf"), 3)
    );
    let plan = vec![PagePlan { page: 2, rotate: 0 }];
    let out = OrganizeInput {
        input: input.clone(),
        plan,
        out: None,
        password: None,
    }
    .run()
    .unwrap()
    .0;
    assert_eq!(
        (out.clone(), n(&out)),
        (dir.path().join("in-organize.pdf"), 1)
    );
    let out = MergeInput {
        inputs: vec![input.clone(), input],
        opts: Default::default(),
        out: None,
        passwords: vec![],
    }
    .run()
    .unwrap()
    .0;
    assert_eq!((out.clone(), n(&out)), (dir.path().join("in-merge.pdf"), 6));
}

#[test]
fn explicit_out_is_honoured() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let want = dir.path().join("custom.pdf");
    let got = ExtractInput {
        input: src(dir.path()),
        pages: vec![0],
        out: Some(want.clone()),
        password: None,
    }
    .run()
    .unwrap()
    .0;
    assert_eq!(got, want);
}

#[test]
fn empty_merge_is_rejected_without_output() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("m.pdf");
    let r = MergeInput {
        inputs: vec![],
        opts: Default::default(),
        out: Some(out.clone()),
        passwords: vec![],
    }
    .run();
    assert!(matches!(r, Err(PagesError::InvalidInput(_))), "{r:?}");
    assert!(!out.exists());
}
