//! Deterministic cancel e2e. Own binary and single test fn: it mutates env.
mod common;
use serde_json::json;
use std::time::{Duration, Instant};

fn wait_for(a: &common::App, id: &serde_json::Value, running: bool) -> serde_json::Value {
    let end = Instant::now() + Duration::from_secs(30);
    loop {
        let j = a.call("job_status", json!({ "job_id": id })).unwrap();
        let kind = j["state"]["kind"].as_str().unwrap();
        let hit = if running {
            kind == "running"
        } else {
            kind != "queued" && kind != "running"
        };
        if hit {
            return j;
        }
        assert!(Instant::now() < end, "timed out: {j}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn cancel_running_job_ends_cancelled_without_output() {
    if !common::have_pdfium() {
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let release = d.path().join("release");
    let bin = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/bin-block");
    // SAFETY: the only test in this binary, set before any other thread exists.
    unsafe {
        std::env::set_var("MONAX_BIN_DIR", bin);
        std::env::set_var("MONAX_TEST_RELEASE", &release);
    }
    let a = common::app();
    let input = common::copy_input(d.path(), "three-pages.pdf");
    let started = a
        .call(
            "pages_extract",
            json!({ "input": { "input": input, "pages": [0] } }),
        )
        .unwrap();
    let id = started["job_id"].clone();
    wait_for(&a, &id, true);
    a.call("job_cancel", json!({ "job_id": id })).unwrap();
    std::fs::write(&release, b"").unwrap();
    let job = wait_for(&a, &id, false);
    assert_eq!(job["state"], json!({ "kind": "cancelled" }), "{job}");
    assert!(job["outputs"].as_array().unwrap().is_empty());
    assert!(!d.path().join("three-pages-extract.pdf").exists());

    // job_cancel on a finished job is a no-op: still cancelled, not rewritten.
    a.call("job_cancel", json!({ "job_id": id })).unwrap();
    assert_eq!(a.call("job_status", json!({ "job_id": id })).unwrap(), job);
}
