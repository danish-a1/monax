//! E2E: OCR over IPC and over MCP through the job registry, and the
//! installed languages each boundary reports.
mod common;
use common::mcp::{Mcp, noop, serve, stop};
use common::*;
use monax_core::Registry;
use serde_json::{Value, json};
use std::path::Path;

const NOTE: &str = "pages were replaced by images with a searchable text layer; \
    links, form fields, annotations and bookmarks were not carried over";
const MISSING: &str = "OCR language not installed: deu";

/// A done job whose one output reads "4471" and ends with `name`.
fn searchable(job: &Value, name: &str) {
    assert_eq!(job["state"]["kind"], "done", "{job}");
    let out = job["outputs"][0].as_str().unwrap();
    assert!(out.ends_with(name), "{out}");
    let text = monax_core::pdfium::extract_text(Path::new(out), 0..=0).unwrap();
    assert!(
        text.contains("INVOICE") && text.contains("4471"),
        "{text:?}"
    );
    assert_eq!(job["notes"], json!([NOTE]), "{job}");
}

fn missing(job: &Value) {
    assert_eq!(job["state"]["kind"], "failed", "{job}");
    assert_eq!(job["state"]["message"], MISSING);
    assert_eq!(job["outputs"], json!([]));
}

#[test]
fn ocr_over_ipc_and_mcp_writes_a_searchable_copy() {
    if !have_tesseract() {
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let input = copy_input(d.path(), "invoice-scan.pdf");
    let a = app();
    let langs = a.call("ocr_languages", json!({})).unwrap();
    assert!(langs.as_array().unwrap().contains(&json!("eng")), "{langs}");
    searchable(
        &a.run_job("ocr_run", json!({ "input": input })),
        "invoice-scan-ocr.pdf",
    );
    missing(&a.run_job("ocr_run", json!({ "input": input, "lang": ["deu"] })));

    let cfg = tempfile::tempdir().unwrap();
    let s = serve(cfg.path(), Registry::new(), noop());
    let m = Mcp::of(&s);
    let caps = m.call("pdf_capabilities", json!({})).unwrap();
    assert_eq!(caps["ocr_languages"], langs);
    searchable(
        &m.run_job("pdf_ocr", json!({ "input": input, "dpi": 150 })),
        "invoice-scan-ocr-1.pdf",
    );
    missing(&m.run_job("pdf_ocr", json!({ "input": input, "lang": ["eng", "deu"] })));
    let e = m.call("pdf_ocr", json!({ "input": input, "pages": [0] }));
    assert!(
        e.unwrap_err()
            .starts_with("invalid arguments: unknown field `pages`")
    );
    stop(s);
    assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 3);
}
