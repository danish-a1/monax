mod common;
use common::*;
use serde_json::{Value, json};

fn done(job: &Value) -> &Value {
    assert_eq!(job["state"]["kind"], "done", "{job}");
    assert_eq!(job["progress"], 1.0);
    &job["outputs"][0]
}

#[test]
fn pages_info_counts_and_rejects() {
    require_env!();
    let a = app();
    let p = fixture("three-pages.pdf");
    let r = a.call("pages_info", json!({ "path": p })).unwrap();
    assert_eq!(r["page_count"], 3);
    let rc = a.call("pages_info", json!({ "path": fixture("rotated-crop.pdf") }));
    let page = &rc.unwrap()["pages"][0];
    assert_eq!(page["rotation"], 90, "{page}");
    assert_eq!(
        page["crop_box"],
        json!([50.0, 20.0, 250.0, 120.0]),
        "{page}"
    );
    assert_eq!(page["media_box"], json!([0.0, 0.0, 300.0, 200.0]), "{page}");
    let e = a
        .call("pages_info", json!({ "path": "relative.pdf" }))
        .unwrap_err();
    assert_eq!(
        e,
        json!("invalid input: path must be absolute: relative.pdf")
    );
}

#[test]
fn pages_thumbnail_returns_png_and_rejects() {
    require_env!();
    let a = app();
    let p = fixture("three-pages.pdf");
    let png = a
        .call_raw(
            "pages_thumbnail",
            json!({ "path": p, "page": 0, "max_px": 2048 }),
        )
        .unwrap();
    assert_eq!(&png[..4], b"\x89PNG");
    let side = |o: usize| u32::from_be_bytes(png[o..o + 4].try_into().unwrap());
    assert_eq!(
        side(16).max(side(20)),
        2048,
        "the viewer renders past 1024 px"
    );
    let e = a
        .call(
            "pages_thumbnail",
            json!({ "path": p, "page": 9, "max_px": 64 }),
        )
        .unwrap_err();
    assert_eq!(
        e,
        json!("page index 9 out of range (document has 3 pages, indices 0-2)")
    );
}

#[test]
fn extract_delete_reorder_rotate_organize() {
    require_env!();
    let a = app();
    let d = tempfile::tempdir().unwrap();
    let input = copy_input(d.path(), "three-pages.pdf");
    let j = a.run_job("pages_extract", json!({ "input": input, "pages": [0, 2] }));
    assert_eq!(pages(done(&j)), 2);
    assert!(
        done(&j)
            .as_str()
            .unwrap()
            .ends_with("three-pages-extract.pdf")
    );
    let j = a.run_job("pages_delete", json!({ "input": input, "pages": [1] }));
    assert_eq!(pages(done(&j)), 2);
    let j = a.run_job(
        "pages_reorder",
        json!({ "input": input, "order": [2, 1, 0] }),
    );
    assert_eq!(pages(done(&j)), 3);
    let j = a.run_job(
        "pages_rotate",
        json!({ "input": input, "pages": [0], "degrees": 90 }),
    );
    assert_eq!(pages(done(&j)), 3);
    let plan = json!([{ "page": 2, "rotate": 90 }, { "page": 0, "rotate": 0 }]);
    let j = a.run_job("pages_organize", json!({ "input": input, "plan": plan }));
    assert_eq!(pages(done(&j)), 2);
}

#[test]
fn merge_with_explicit_out() {
    require_env!();
    let a = app();
    let d = tempfile::tempdir().unwrap();
    let input = copy_input(d.path(), "three-pages.pdf");
    let out = d.path().join("merged.pdf");
    let j = a.run_job(
        "pages_merge",
        json!({ "inputs": [input, input], "opts": { "blank_before_even": false }, "out": out }),
    );
    assert_eq!(done(&j).as_str().unwrap(), out.to_str().unwrap());
    assert_eq!(pages(done(&j)), 6);
}

#[test]
fn job_status_unknown_is_null_and_cancel_is_void() {
    let a = app();
    assert_eq!(
        a.call("job_status", json!({ "job_id": "999999" })).unwrap(),
        Value::Null
    );
    assert_eq!(
        a.call("job_cancel", json!({ "job_id": "999999" })).unwrap(),
        Value::Null
    );
    let e = a
        .call("job_status", json!({ "job_id": "abc" }))
        .unwrap_err();
    assert_eq!(
        e,
        json!("invalid args `job_id` for command `job_status`: invalid digit found in string")
    );
}

#[test]
fn cancel_on_finished_job_is_a_noop() {
    require_env!();
    let a = app();
    let d = tempfile::tempdir().unwrap();
    let input = copy_input(d.path(), "three-pages.pdf");
    let id = a
        .call(
            "pages_extract",
            json!({ "input": { "input": input, "pages": [0] } }),
        )
        .unwrap()["job_id"]
        .clone();
    let job = loop {
        let j = a.call("job_status", json!({ "job_id": id })).unwrap();
        if j["state"]["kind"] == "done" {
            break j;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    a.call("job_cancel", json!({ "job_id": id })).unwrap();
    assert_eq!(a.call("job_status", json!({ "job_id": id })).unwrap(), job);
    assert!(d.path().join("three-pages-extract.pdf").exists());
}
