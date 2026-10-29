//! E2E (R112 - R115): a form whose field tree revisits a field is refused
//! before pdfium draws or walks it, a file qpdf warns on and one whose
//! kept page would change are refused for redaction, through Tauri
//! commands and MCP tools, with the exact reason and no output; a repair
//! notes that it may display differently.
mod common;
use common::mcp::{Mcp, noop, serve, stop};
use common::*;
use monax_core::Registry;
use serde_json::{Value, json};

const REVISIT: &str = "damaged file: form field tree reaches a field twice";
const REPAIR: &str = "damaged file: repair it first, then redact";
const CHANGED: &str = "redaction check failed: a kept page would change";
const REPAIRED: &str = "repaired file may display differently from the damaged original";

fn failed(job: &Value) -> &str {
    assert_eq!(job["state"]["kind"], "failed", "{job}");
    assert_eq!(job["outputs"], json!([]));
    job["state"]["message"].as_str().unwrap()
}

fn redact_args(input: &str) -> Value {
    let rect = json!({ "left": 10, "bottom": 10, "right": 50, "top": 50 });
    json!({ "input": input, "boxes": [{ "page": 1, "rect": rect }] })
}

#[test]
fn commands_refuse_what_they_cannot_draw_or_redact_faithfully() {
    require_env!();
    let a = app();
    let d = tempfile::tempdir().unwrap();
    let dag = copy_input(d.path(), "field-dag.pdf");
    let args = json!({ "path": dag, "page": 0, "max_px": 64 });
    assert_eq!(a.call("pages_thumbnail", args), Err(json!(REVISIT)));
    let junk = copy_input(d.path(), "ocmd-junk.pdf");
    let before = std::fs::read_dir(d.path()).unwrap().count();
    assert_eq!(
        failed(&a.run_job("security_redact", redact_args(&dag))),
        REVISIT
    );
    assert_eq!(
        failed(&a.run_job("security_redact", redact_args(&junk))),
        REPAIR
    );
    let gen1 = copy_input(d.path(), "ocmd-gen1.pdf");
    assert_eq!(
        failed(&a.run_job("security_redact", redact_args(&gen1))),
        CHANGED
    );
    let job = a.run_job("content_delete_annotations", json!({ "input": dag }));
    assert_eq!(failed(&job), REVISIT);
    assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), before + 1);
    let job = a.run_job("content_repair", json!({ "input": junk }));
    assert_eq!(job["state"]["kind"], "done", "{job}");
    assert_eq!(job["notes"], json!([REPAIRED]));
}

#[test]
fn mcp_tools_refuse_what_they_cannot_draw_or_redact_faithfully() {
    require_env!();
    let d = tempfile::tempdir().unwrap();
    let s = serve(&d.path().join("cfg"), Registry::new(), noop());
    let m = Mcp::of(&s);
    let dag = copy_input(d.path(), "field-dag.pdf");
    let junk = copy_input(d.path(), "ocmd-junk.pdf");
    let before = std::fs::read_dir(d.path()).unwrap().count();
    let job = m.run_job("pdf_render_page", json!({ "input": dag, "page": 0 }));
    assert_eq!(failed(&job), REVISIT);
    assert_eq!(failed(&m.run_job("pdf_redact", redact_args(&junk))), REPAIR);
    let gen1 = copy_input(d.path(), "ocmd-gen1.pdf");
    assert_eq!(
        failed(&m.run_job("pdf_redact", redact_args(&gen1))),
        CHANGED
    );
    assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), before + 1);
    let job = m.run_job("pdf_repair", json!({ "input": junk }));
    assert_eq!(job["state"]["kind"], "done", "{job}");
    assert_eq!(job["notes"], json!([REPAIRED]));
    stop(s);
}
