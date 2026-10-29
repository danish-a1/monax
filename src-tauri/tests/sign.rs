//! E2E: signature image and text placed over IPC and over MCP through the
//! job registry; each job notes it is not a digital signature.
mod common;
use common::mcp::{Mcp, noop, serve, stop};
use common::*;
use monax_core::Registry;
use monax_core::forms::SIGN_NOTE;
use monax_core::pdfium::{extract_text, render};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const OUTSIDE: &str = "invalid input: signature rect is outside page 0";

fn placement(left: u32) -> Value {
    json!({ "page": 0, "rect": { "left": left, "bottom": 20, "right": left + 100, "top": 60 } })
}

/// The done job's one output, which ends with `file`; its notes are
/// exactly [`SIGN_NOTE`].
fn signed(job: &Value, file: &str) -> PathBuf {
    assert_eq!(job["state"]["kind"], "done", "{job}");
    assert_eq!(job["notes"], json!([SIGN_NOTE]), "{job}");
    let out = PathBuf::from(job["outputs"][0].as_str().unwrap());
    assert!(out.ends_with(file), "{out:?}");
    out
}

fn failed(job: &Value) -> &str {
    assert_eq!(job["state"]["kind"], "failed", "{job}");
    assert_eq!(job["outputs"], json!([]));
    job["state"]["message"].as_str().unwrap()
}

/// True when page 0 of `a` and `b` differ at displayed pixel `(x, y)`.
fn inked(a: &Path, b: &Path, (x, y): (u32, u32)) -> bool {
    let (ra, rb) = (render(a, 0, 72).unwrap(), render(b, 0, 72).unwrap());
    let i = ((y * ra.w + x) * 4) as usize;
    ra.rgba[i..i + 4] != rb.rgba[i..i + 4]
}

#[test]
fn sign_over_ipc_places_image_and_text() {
    require_env!();
    let d = tempfile::tempdir().unwrap();
    let input = copy_input(d.path(), "three-pages.pdf");
    let image = copy_input(d.path(), "signature.png");
    let a = app();
    let job = a.run_job(
        "forms_sign_image",
        json!({ "input": input, "image": image, "placement": placement(60) }),
    );
    let out = signed(&job, "three-pages-signed.pdf");
    assert!(inked(Path::new(&input), &out, (110, 176)), "no image ink");
    let job = a.run_job(
        "forms_sign_text",
        json!({ "input": input, "text": "Ann", "font_size": 20, "placement": placement(60) }),
    );
    let out = signed(&job, "three-pages-signed-1.pdf");
    assert!(extract_text(&out, 0..=0).unwrap().contains("Ann"));
    let bad =
        json!({ "input": input, "text": "Ann", "font_size": 20, "placement": placement(150) });
    assert_eq!(failed(&a.run_job("forms_sign_text", bad)), OUTSIDE);
    assert_eq!(
        names(d.path()),
        [
            "signature.png",
            "three-pages-signed-1.pdf",
            "three-pages-signed.pdf",
            "three-pages.pdf"
        ]
    );
}

#[test]
fn sign_over_mcp_places_image_and_text() {
    require_env!();
    let d = tempfile::tempdir().unwrap();
    let input = copy_input(d.path(), "three-pages.pdf");
    let image = copy_input(d.path(), "signature.png");
    let s = serve(&d.path().join("cfg"), Registry::new(), noop());
    let m = Mcp::of(&s);
    let out = d.path().join("img.pdf");
    let args = json!({ "input": input, "image": image, "placement": placement(60), "out": out });
    signed(&m.run_job("pdf_sign_image", args), "img.pdf");
    assert!(inked(Path::new(&input), &out, (110, 176)), "no image ink");
    let args = json!({ "input": input, "text": "Bo", "font_size": 20, "placement": placement(60) });
    let out = signed(&m.run_job("pdf_sign_text", args), "three-pages-signed.pdf");
    assert!(extract_text(&out, 0..=0).unwrap().contains("Bo"));
    let bad = json!({ "input": input, "image": image, "placement": placement(150) });
    assert_eq!(failed(&m.run_job("pdf_sign_image", bad)), OUTSIDE);
    let tools = m.rpc("tools/list", json!({}))["tools"].clone();
    for name in ["pdf_sign_image", "pdf_sign_text"] {
        let t = tools.as_array().unwrap().iter().find(|t| t["name"] == name);
        let about = t.unwrap()["description"].as_str().unwrap();
        assert!(about.contains(SIGN_NOTE), "{name}: {about}");
    }
    let e = m.call("pdf_sign_text", json!({ "input": input, "text": "x" }));
    assert!(
        e.unwrap_err()
            .starts_with("invalid arguments: missing field `font_size`")
    );
    stop(s);
    assert_eq!(
        names(d.path()),
        [
            "cfg",
            "img.pdf",
            "signature.png",
            "three-pages-signed.pdf",
            "three-pages.pdf"
        ]
    );
}
