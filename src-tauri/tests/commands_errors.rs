mod common;
use common::*;
use serde_json::{Value, json};

fn failed(job: &Value) -> &str {
    assert_eq!(job["state"]["kind"], "failed", "{job}");
    assert!(job["outputs"].as_array().unwrap().is_empty());
    job["state"]["message"].as_str().unwrap()
}

#[test]
fn op_failures_surface_the_core_message() {
    require_env!();
    let a = app();
    let d = tempfile::tempdir().unwrap();
    let input = copy_input(d.path(), "three-pages.pdf");
    let enc = copy_input(d.path(), "encrypted.pdf");
    let cases = [
        (
            "pages_extract",
            json!({ "input": "rel.pdf", "pages": [0] }),
            "invalid input: path must be absolute: rel.pdf",
        ),
        (
            "pages_delete",
            json!({ "input": input, "pages": [7] }),
            "page index 7 out of range (document has 3 pages, indices 0-2)",
        ),
        (
            "pages_reorder",
            json!({ "input": input, "order": [9] }),
            "page index 9 out of range (document has 3 pages, indices 0-2)",
        ),
        (
            "pages_rotate",
            json!({ "input": input, "pages": [0], "degrees": 45 }),
            "invalid input: rotation must be a non-zero multiple of 90, got 45",
        ),
        (
            "pages_organize",
            json!({ "input": input, "plan": [{ "page": 5, "rotate": 0 }] }),
            "page index 5 out of range (document has 3 pages, indices 0-2)",
        ),
        (
            "pages_merge",
            json!({ "inputs": [] }),
            "invalid input: no inputs to merge",
        ),
        (
            "pages_extract",
            json!({ "input": enc, "pages": [0] }),
            "encrypted input: a password is required",
        ),
    ];
    for (cmd, args, want) in cases {
        assert_eq!(failed(&a.run_job(cmd, args)), want, "{cmd}");
    }
}
