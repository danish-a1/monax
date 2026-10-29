//! MCP inside the app: the server shares the GUI's job registry, Settings
//! (`mcp_info`) reports its state, and `launch` wires `pdf_open_in_app` to the
//! `open-document` event.
mod common;
use common::mcp::{Mcp, noop, serve, stop};
use common::*;
use monax_core::Registry;
use monax_lib::mcp::{McpState, McpStatus, launch};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::mpsc;
use tauri::{Listener, Manager};

fn done(job: &Value) -> PathBuf {
    assert_eq!(job["state"]["kind"], "done", "{job}");
    PathBuf::from(job["outputs"][0].as_str().unwrap())
}

#[test]
fn gui_and_mcp_share_one_registry_and_settings_see_the_server() {
    require_env!();
    let a = app();
    let d = tempfile::tempdir().unwrap();
    let reg = a.app.state::<Registry>().inner().clone();
    let s = serve(&d.path().join("cfg"), reg, noop());
    a.app
        .state::<McpState>()
        .set(McpStatus::Running(s.info.clone()));
    let info = a.call("mcp_info", json!({})).unwrap();
    assert_eq!(
        info,
        json!({ "status": "running", "url": s.info.url, "token": s.info.token,
                "port_changed": false })
    );

    let m = Mcp::of(&s);
    let input = copy_input(d.path(), "three-pages.pdf");
    let gui = a.call("pages_info", json!({ "path": input })).unwrap();
    assert_eq!(gui["page_count"], 3);
    let id = m.call("pdf_page_info", json!({ "input": input })).unwrap()["job_id"].clone();
    let job = loop {
        let j = a.call("job_status", json!({ "job_id": id })).unwrap();
        if j["state"]["kind"] == "done" {
            break j;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    let via_mcp = m.call("pdf_job_status", json!({ "job_id": id })).unwrap();
    assert_eq!(
        via_mcp, job,
        "GUI sees the MCP job, and MCP the same snapshot"
    );
    let gui = a.call(
        "pages_extract",
        json!({ "input": { "input": input, "pages": [0] } }),
    );
    let id = gui.unwrap()["job_id"].clone();
    let out = loop {
        let j = m.call("pdf_job_status", json!({ "job_id": id })).unwrap();
        if j["state"]["kind"] == "done" {
            break done(&j);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    assert_eq!(pages(&json!(out)), 1, "MCP sees the GUI job");
    stop(s);
}

#[test]
fn settings_report_starting_before_launch() {
    let a = app();
    assert_eq!(
        a.call("mcp_info", json!({})).unwrap(),
        json!({ "status": "starting" })
    );
}

#[test]
fn launch_serves_and_open_in_app_emits_open_document() {
    let a = app();
    let d = tempfile::tempdir().unwrap();
    let (tx, rx) = mpsc::channel();
    a.app.listen("open-document", move |e| {
        tx.send(e.payload().to_string()).unwrap()
    });
    launch(a.app.handle(), Some(d.path().join("cfg")));
    let info = a.call("mcp_info", json!({})).unwrap();
    assert_eq!(info["status"], "running", "{info}");
    let m = Mcp::new(
        info["url"].as_str().unwrap(),
        info["token"].as_str().unwrap(),
    );
    let p = fixture("three-pages.pdf");
    m.call("pdf_open_in_app", json!({ "path": p })).unwrap();
    let got = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
    assert_eq!(got, json!(p).to_string());
}

#[test]
fn settings_flag_a_moved_port_with_a_rotated_token() {
    let a = app();
    let d = tempfile::tempdir().unwrap();
    let held = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let old_url = format!("http://127.0.0.1:{}/mcp", held.local_addr().unwrap().port());
    let old_token = "a".repeat(64);
    let cfg = d.path().join("cfg");
    std::fs::create_dir(&cfg).unwrap();
    let stored = json!({ "url": old_url, "token": old_token });
    std::fs::write(cfg.join("mcp.json"), stored.to_string()).unwrap();
    launch(a.app.handle(), Some(cfg));
    let info = a.call("mcp_info", json!({})).unwrap();
    assert_eq!(
        (&info["status"], &info["port_changed"]),
        (&json!("running"), &json!(true))
    );
    assert_ne!(info["url"], json!(old_url));
    assert_ne!(info["token"], json!(old_token));
}

#[test]
fn launch_failure_is_reported_with_the_reason() {
    let a = app();
    launch(a.app.handle(), None);
    let info = a.call("mcp_info", json!({})).unwrap();
    assert_eq!(
        info,
        json!({ "status": "failed", "reason": "no config directory: HOME unset or MONAX_CONFIG_DIR not absolute" })
    );
    let b = app();
    let d = tempfile::tempdir().unwrap();
    let file = d.path().join("f");
    std::fs::write(&file, "").unwrap();
    launch(b.app.handle(), Some(file.join("cfg")));
    let info = b.call("mcp_info", json!({})).unwrap();
    assert_eq!(info["status"], "failed");
    let reason = info["reason"].as_str().unwrap();
    assert!(
        reason.ends_with(&format!("({})", file.join("cfg").display())),
        "{reason}"
    );
}
