//! E2E: Office to PDF over IPC and over MCP through the job registry,
//! driven by the fake `wordcraft-cli`; each job's output is the file on
//! disk, or the exact error. A missing sidecar greys the tool out and fails
//! the job. PDF to Office is gone (R152): the IPC command here, the MCP tool
//! from the tool list (tests/mcp.rs).
mod common;
use common::mcp::{Mcp, noop, serve, stop};
use common::*;
use monax_core::Registry;
use serde_json::{Value, json};
use std::path::PathBuf;

fn output(job: &Value) -> PathBuf {
    assert_eq!(job["state"]["kind"], "done", "{job}");
    let p = PathBuf::from(job["outputs"][0].as_str().unwrap());
    assert!(p.is_file(), "{p:?}");
    p
}

// Mutates process env (the fake wordcraft-cli): the only test in this binary.
#[test]
fn office_conversion_over_ipc_and_mcp() {
    require_env!();
    let logs = tempfile::tempdir().unwrap();
    // SAFETY: the only test in this binary.
    unsafe {
        std::env::set_var("MONAX_BIN_DIR", fixture("bin-wordcraft"));
        std::env::set_var("FAKE_WORDCRAFT_LOG", logs.path().join("log"));
    }
    let d = tempfile::tempdir().unwrap();
    let doc = d.path().join("memo.docx");
    std::fs::write(&doc, b"PK\x03\x04 memo").unwrap();

    let a = app();
    assert_eq!(a.call("office_available", json!({})).unwrap(), json!(true));
    let job = a.run_job("convert_office_to_pdf", json!({ "input": doc }));
    assert_eq!(output(&job), d.path().join("memo.pdf"));
    assert!(job.get("notes").is_none(), "{job}");
    let gone = a.call(
        "convert_pdf_to_office",
        json!({ "input": { "input": doc } }),
    );
    let unknown = "Command convert_pdf_to_office not found";
    assert_eq!(gone.unwrap_err(), json!(unknown));

    let s = serve(&d.path().join("cfg"), Registry::new(), noop());
    let m = Mcp::of(&s);
    let caps = m.call("pdf_capabilities", json!({})).unwrap();
    assert_eq!(caps["wordcraft"], "wordcraft-cli 0.12.0 fake");
    assert!(caps.get("libreoffice").is_none(), "{caps}");
    let job = m.run_job("pdf_office_to_pdf", json!({ "input": doc }));
    assert_eq!(output(&job), d.path().join("memo-1.pdf"));
    // SAFETY: as above.
    unsafe { std::env::set_var("FAKE_WORDCRAFT", "none") };
    let job = m.run_job("pdf_office_to_pdf", json!({ "input": doc }));
    assert_eq!(job["state"]["message"], "WordCraft produced no output");

    // Missing sidecar: never looked up on PATH.
    // SAFETY: as above.
    unsafe {
        std::env::set_var("MONAX_BIN_DIR", "/nonexistent");
        std::env::set_var("PATH", fixture("bin-wordcraft"));
    }
    let caps = m.call("pdf_capabilities", json!({})).unwrap();
    assert_eq!(caps["wordcraft"], Value::Null);
    let missing = "WordCraft not found - the bundled converter is missing";
    let job = m.run_job("pdf_office_to_pdf", json!({ "input": doc }));
    assert_eq!(job["state"]["message"], missing);
    stop(s);
    assert_eq!(a.call("office_available", json!({})).unwrap(), json!(false));
    let job = a.run_job("convert_office_to_pdf", json!({ "input": doc }));
    assert_eq!(job["state"]["message"], missing);
    let want = ["cfg", "memo-1.pdf", "memo.docx", "memo.pdf"];
    assert_eq!(names(d.path()), want);
}
