//! E2E: every op command over IPC through the job registry: output files on
//! disk with the right page counts, or the exact failure and no file.
mod common;
use common::ops::{check_success, failures, noted, output_pages, successes};
use common::*;
use serde_json::json;

#[test]
fn every_op_command_writes_its_outputs() {
    require_env!();
    let a = app();
    let d = tempfile::tempdir().unwrap();
    for c in successes(d.path()) {
        let job = a.run_job(c.cmd, c.args.clone());
        check_success(&c, &job);
    }
}

#[test]
fn every_new_op_command_fails_with_the_core_message() {
    require_env!();
    let a = app();
    let d = tempfile::tempdir().unwrap();
    let cases = failures(d.path());
    let before = std::fs::read_dir(d.path()).unwrap().count();
    for (_, cmd, args, want) in cases {
        let job = a.run_job(cmd, args);
        assert_eq!(job["state"]["kind"], "failed", "{cmd}: {job}");
        assert!(!job.to_string().contains("Sesame"), "{cmd}: secret echoed");
        assert_eq!(
            (job["state"]["message"].as_str(), &job["outputs"]),
            (Some(want), &json!([])),
            "{cmd}"
        );
    }
    assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), before);
}

#[test]
fn malformed_input_is_rejected_at_the_boundary() {
    let a = app();
    let e = a
        .call(
            "pages_split_pages",
            json!({ "input": { "input": "/a.pdf", "ranges": [], "x": 1 } }),
        )
        .unwrap_err();
    let e = e.as_str().unwrap();
    assert!(
        e.starts_with("invalid args `input` for command `pages_split_pages`: unknown field `x`"),
        "{e}"
    );
}

#[test]
fn job_status_carries_the_notes_of_an_op() {
    require_env!();
    let a = app();
    let d = tempfile::tempdir().unwrap();
    let (_, cmd, args, notes) = noted(d.path());
    let job = a.run_job(cmd, args);
    assert_eq!(output_pages(&job), [1]);
    assert_eq!(job["notes"], notes, "{job}");
    let plain = a.run_job(
        cmd,
        json!({ "input": copy_input(d.path(), "three-pages.pdf") }),
    );
    assert!(plain.get("notes").is_none(), "{plain}");
}
