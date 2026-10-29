//! Mock-runtime IPC harness: invokes commands with the JSON the frontend sends.
#![allow(dead_code)]
pub mod mcp;
pub mod ops;
use monax_lib::configure;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tauri::ipc::InvokeBody;
use tauri::test::{
    INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
};
use tauri::webview::InvokeRequest;
use tauri::{WebviewWindow, WebviewWindowBuilder};

pub fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../crates/monax-core/tests/fixtures")
        .join(name)
}

/// Skip-or-strict, same policy as monax-core's helpers.
pub fn have_pdfium() -> bool {
    let ok = std::env::var_os("MONAX_LIBPDFIUM").is_some_and(|p| Path::new(&p).is_file());
    if !ok {
        let strict =
            std::env::var_os("CI").is_some() || std::env::var_os("MONAX_REQUIRE_PDFIUM").is_some();
        let msg = "SKIPPED: pdfium missing (strict: CI, MONAX_REQUIRE_PDFIUM)\n";
        assert!(!strict, "{msg}");
        let _ = std::io::Write::write_all(&mut std::io::stderr(), msg.as_bytes());
    }
    ok
}

pub fn available() -> bool {
    let pdfium = std::env::var_os("MONAX_LIBPDFIUM").is_some_and(|p| Path::new(&p).is_file());
    let qpdf = monax_sidecar::find(monax_sidecar::Tool::Qpdf).is_some();
    let ok = pdfium && qpdf;
    if !ok {
        let strict = std::env::var_os("CI").is_some()
            || std::env::var_os("MONAX_REQUIRE_PDFIUM").is_some()
            || std::env::var_os("MONAX_REQUIRE_SIDECARS").is_some();
        let msg = "SKIPPED: pdfium or qpdf missing (strict: CI, MONAX_REQUIRE_PDFIUM, MONAX_REQUIRE_SIDECARS)\n";
        assert!(!strict, "{msg}");
        let _ = std::io::Write::write_all(&mut std::io::stderr(), msg.as_bytes());
    }
    ok
}

/// [`available`] and tesseract found (strict as [`available`]).
pub fn have_tesseract() -> bool {
    let ok = monax_sidecar::find(monax_sidecar::Tool::Tesseract).is_some();
    if !ok {
        let strict = std::env::var_os("CI").is_some()
            || std::env::var_os("MONAX_REQUIRE_SIDECARS").is_some();
        let msg = "SKIPPED: tesseract missing (strict: CI, MONAX_REQUIRE_SIDECARS)\n";
        assert!(!strict, "{msg}");
        let _ = std::io::Write::write_all(&mut std::io::stderr(), msg.as_bytes());
    }
    available() && ok
}

#[macro_export]
macro_rules! require_env {
    () => {
        if !common::available() {
            return;
        }
    };
}

pub struct App {
    pub app: tauri::App<MockRuntime>,
    pub win: WebviewWindow<MockRuntime>,
}

pub fn app() -> App {
    let app = configure(mock_builder())
        .build(mock_context(noop_assets()))
        .unwrap();
    let win = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    App { app, win }
}

impl App {
    fn request(&self, cmd: &str, args: Value) -> Result<tauri::ipc::InvokeResponseBody, Value> {
        get_ipc_response(
            &self.win,
            InvokeRequest {
                cmd: cmd.into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().unwrap(),
                body: InvokeBody::Json(args),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
    }

    /// JSON result or the rejection value.
    pub fn call(&self, cmd: &str, args: Value) -> Result<Value, Value> {
        self.request(cmd, args).map(|b| b.deserialize().unwrap())
    }

    /// Raw bytes of a binary response (thumbnails).
    pub fn call_raw(&self, cmd: &str, args: Value) -> Result<Vec<u8>, Value> {
        self.request(cmd, args).map(|b| match b {
            tauri::ipc::InvokeResponseBody::Raw(v) => v,
            other => panic!("expected raw bytes, got {other:?}"),
        })
    }

    /// Starts an op and polls `job_status` over IPC until terminal.
    pub fn run_job(&self, cmd: &str, input: Value) -> Value {
        let id = self.call(cmd, json!({ "input": input })).unwrap()["job_id"].clone();
        let end = Instant::now() + Duration::from_secs(30);
        loop {
            let job = self.call("job_status", json!({ "job_id": id })).unwrap();
            if !matches!(job["state"]["kind"].as_str(), Some("queued" | "running")) {
                return job;
            }
            assert!(Instant::now() < end, "job did not finish: {job}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// Sorted entry names of `dir`.
pub fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

pub fn copy_input(dir: &Path, name: &str) -> String {
    let p = dir.join(name);
    std::fs::copy(fixture(name), &p).unwrap();
    p.to_string_lossy().into_owned()
}

pub fn pages(v: &Value) -> usize {
    monax_core::pdfium::page_count(Path::new(v.as_str().unwrap())).unwrap()
}
