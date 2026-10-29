//! E2E with the real `wordcraft-cli` (built into `sidecars/` by
//! scripts/fetch-sidecars.sh): a docx becomes a PDF over IPC and over MCP.
//! Skips with a notice when it is not built.
mod common;
use common::mcp::{Mcp, noop, serve, stop};
use common::*;
use monax_core::Registry;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn pdf(job: &Value) -> PathBuf {
    assert_eq!(job["state"]["kind"], "done", "{job}");
    let p = PathBuf::from(job["outputs"][0].as_str().unwrap());
    let text = monax_core::pdfium::extract_text(&p, 0..=0).unwrap();
    assert!(text.contains("Quokka"), "{text:?}");
    p
}

// Mutates process env: the only test in this binary.
#[test]
fn real_wordcraft_over_ipc_and_mcp() {
    require_env!();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../sidecars");
    if !dir.join("wordcraft-cli").is_file() {
        let msg = "SKIPPED: wordcraft-cli not built (scripts/fetch-sidecars.sh)\n";
        let _ = std::io::Write::write_all(&mut std::io::stderr(), msg.as_bytes());
        return;
    }
    // SAFETY: the only test in this binary.
    unsafe { std::env::set_var("MONAX_BIN_DIR", &dir) };
    let d = tempfile::tempdir().unwrap();
    let doc = d.path().join("hello.docx");
    std::fs::copy(fixture("office/hello.docx"), &doc).unwrap();
    let job = app().run_job("convert_office_to_pdf", json!({ "input": doc }));
    assert_eq!(pdf(&job), d.path().join("hello.pdf"));
    let s = serve(&d.path().join("cfg"), Registry::new(), noop());
    let job = Mcp::of(&s).run_job("pdf_office_to_pdf", json!({ "input": doc }));
    assert_eq!(pdf(&job), d.path().join("hello-1.pdf"));
    stop(s);
}
