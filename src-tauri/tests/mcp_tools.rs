//! MCP tools e2e over real HTTP: each tool submits, is polled through
//! `pdf_job_status`, and leaves its output on disk (or the exact error).
mod common;
use common::mcp::{Mcp, noop, serve, stop};
use common::*;
use monax_core::Registry;
use monax_lib::mcp::OpenHook;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn done(job: &Value) -> PathBuf {
    assert_eq!(job["state"]["kind"], "done", "{job}");
    PathBuf::from(job["outputs"][0].as_str().unwrap())
}

fn failed(job: &Value) -> &str {
    assert_eq!(job["state"]["kind"], "failed", "{job}");
    assert_eq!(job["outputs"], json!([]));
    job["state"]["message"].as_str().unwrap()
}

#[test]
fn capabilities_report_the_sidecars() {
    let d = tempfile::tempdir().unwrap();
    let s = serve(d.path(), Registry::new(), noop());
    let caps = Mcp::of(&s).call("pdf_capabilities", json!({})).unwrap();
    let c = monax_sidecar::Capabilities::detect();
    let v = |o: Option<monax_sidecar::Version>| json!(o.map(|v| v.0));
    let want = json!({ "qpdf": v(c.qpdf), "tesseract": v(c.tesseract),
        "wordcraft": v(c.wordcraft), "ocr_languages": monax_core::ocr::languages() });
    assert_eq!(caps, want);
    let e = Mcp::of(&s)
        .call("pdf_capabilities", json!({ "x": 1 }))
        .unwrap_err();
    assert!(e.starts_with("invalid arguments: unknown field `x`"), "{e}");
    stop(s);
}

#[test]
fn unknown_tools_are_a_protocol_error() {
    let d = tempfile::tempdir().unwrap();
    let s = serve(d.path(), Registry::new(), noop());
    let msg = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                      "params": { "name": "pdf_nope", "arguments": {} } });
    let (status, body) = Mcp::of(&s).raw(&[], &msg);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        (status, &v["error"]["message"]),
        (200, &json!("unknown tool: pdf_nope"))
    );
    stop(s);
}

#[test]
fn job_cancel_cancels_a_running_job() {
    let d = tempfile::tempdir().unwrap();
    let reg = Registry::new();
    let (started_tx, started) = std::sync::mpsc::channel();
    let id = reg.submit(move |p| {
        started_tx.send(()).unwrap();
        let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !p.is_cancelled() && std::time::Instant::now() < end {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        Ok::<_, String>(vec![])
    });
    let s = serve(d.path(), reg, noop());
    let m = Mcp::of(&s);
    let id = json!(id);
    started
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("job closure never started");
    let before = m.call("pdf_job_status", json!({ "job_id": id })).unwrap();
    assert_eq!(before["state"]["kind"], "running", "{before}");
    assert_eq!(
        m.call("pdf_job_cancel", json!({ "job_id": id })),
        Ok(json!({ "cancel_requested": true }))
    );
    let job = loop {
        let j = m.call("pdf_job_status", json!({ "job_id": id })).unwrap();
        if !matches!(j["state"]["kind"].as_str(), Some("queued" | "running")) {
            break j;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    assert_eq!(job["state"]["kind"], "cancelled", "{job}");
    assert_eq!(job["outputs"], json!([]));
    stop(s);
}

#[test]
fn job_control_rejects_unknown_ids() {
    let d = tempfile::tempdir().unwrap();
    let s = serve(d.path(), Registry::new(), noop());
    let m = Mcp::of(&s);
    for tool in ["pdf_job_status", "pdf_job_cancel"] {
        assert_eq!(
            m.call(tool, json!({ "job_id": "999" })),
            Err("unknown job_id".into())
        );
        let e = m.call(tool, json!({ "job_id": "x" })).unwrap_err();
        assert!(e.starts_with("invalid arguments:"), "{e}");
    }
    stop(s);
}

#[test]
fn open_in_app_validates_then_calls_the_hook() {
    let d = tempfile::tempdir().unwrap();
    let seen = Arc::new(Mutex::new(Vec::<PathBuf>::new()));
    let log = seen.clone();
    let hook: OpenHook = Arc::new(move |p| log.lock().unwrap().push(p.into()));
    let s = serve(d.path(), Registry::new(), hook);
    let m = Mcp::of(&s);
    let p = fixture("three-pages.pdf");
    assert_eq!(
        m.call("pdf_open_in_app", json!({ "path": p })),
        Ok(json!({ "opened": p }))
    );
    let e = m
        .call("pdf_open_in_app", json!({ "path": "rel.pdf" }))
        .unwrap_err();
    assert_eq!(e, "invalid input: path must be absolute: rel.pdf");
    let missing = d.path().join("nope.pdf");
    let e = m
        .call("pdf_open_in_app", json!({ "path": missing }))
        .unwrap_err();
    assert_eq!(
        e,
        format!("invalid input: file does not exist: {}", missing.display())
    );
    assert_eq!(*seen.lock().unwrap(), [p]);
    stop(s);
}

#[test]
fn read_tools_write_their_outputs() {
    if !have_pdfium() {
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let s = serve(&d.path().join("cfg"), Registry::new(), noop());
    let m = Mcp::of(&s);
    let input = copy_input(d.path(), "three-pages.pdf");
    let out = done(&m.run_job("pdf_page_info", json!({ "input": input })));
    assert_eq!(out, d.path().join("three-pages-page-info.json"));
    let info: Value = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
    assert_eq!(info["page_count"], 3);
    let out = done(&m.run_job("pdf_read_text", json!({ "input": input, "pages": [1] })));
    let want = monax_core::pdfium::extract_text(std::path::Path::new(&input), 1..=1).unwrap();
    assert_eq!(std::fs::read_to_string(&out).unwrap(), want);
    let png = d.path().join("p.png");
    let job = m.run_job(
        "pdf_render_page",
        json!({ "input": input, "page": 0, "max_px": 64, "out": png }),
    );
    assert_eq!(done(&job), png);
    assert_eq!(&std::fs::read(&png).unwrap()[..4], b"\x89PNG");
    let job = m.run_job("pdf_render_page", json!({ "input": input, "page": 7 }));
    assert_eq!(
        failed(&job),
        "page index 7 out of range (document has 3 pages, indices 0-2)"
    );
    let cancel = m.call("pdf_job_cancel", json!({ "job_id": job["id"] }));
    assert_eq!(cancel, Ok(json!({ "cancel_requested": true })));
    let after = m
        .call("pdf_job_status", json!({ "job_id": job["id"] }))
        .unwrap();
    assert_eq!(after, job, "cancelling a finished job changes nothing");
    let e = m
        .call("pdf_read_text", json!({ "input": input, "bogus": 1 }))
        .unwrap_err();
    assert!(
        e.starts_with("invalid arguments: unknown field `bogus`"),
        "{e}"
    );
    stop(s);
}
