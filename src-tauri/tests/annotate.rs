//! E2E: annotations added over IPC (`annotate_save`) and over MCP
//! (`pdf_annotate`) through the job registry; the output carries each
//! annotation with an appearance stream that pdfium draws.
mod common;
use common::mcp::{Mcp, noop, serve, stop};
use common::*;
use monax_core::Registry;
use monax_core::pdfium::render;
use monax_sidecar::Tool;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

const OUTSIDE: &str = "invalid input: annotation 1: rect is outside the page's MediaBox";

fn rect(left: u32, bottom: u32) -> Value {
    json!({ "left": left, "bottom": bottom, "right": left + 60, "top": bottom + 30 })
}

/// A highlight at (60, 20) and a rectangle at `shape_left` on page 0.
fn annots(shape_left: u32) -> Value {
    json!([
        { "page": 0, "annot": { "type": "highlight", "rect": rect(60, 20), "color": "#FFFF00" } },
        { "page": 0, "annot": { "type": "shape", "kind": "rectangle", "rect": rect(shape_left, 120),
          "color": "#0000FF", "width": 2 } },
    ])
}

fn done(job: &Value, file: &str) -> PathBuf {
    assert_eq!(job["state"]["kind"], "done", "{job}");
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

/// /Subtype and whether /AP is present, of every annotation on page 0.
fn written(p: &Path) -> Vec<(String, bool)> {
    let args = ["--json=2".into(), "--json-key=qpdf".into(), p.into()];
    let never = AtomicBool::new(false);
    let out = monax_sidecar::run(Tool::Qpdf, &args, Duration::from_secs(60), &never).unwrap();
    let v: Value = serde_json::from_str(&out.stdout).unwrap();
    let objs = v["qpdf"][1].as_object().unwrap();
    let mut found: Vec<(String, bool)> = objs
        .values()
        .filter_map(|o| o["value"].as_object())
        .filter(|d| d.get("/Type") == Some(&json!("/Annot")))
        .map(|d| {
            (
                d["/Subtype"].as_str().unwrap().to_string(),
                d.contains_key("/AP"),
            )
        })
        .collect();
    found.sort();
    found
}

#[test]
fn annotate_over_ipc_writes_visible_annotations() {
    require_env!();
    let d = tempfile::tempdir().unwrap();
    let input = copy_input(d.path(), "three-pages.pdf");
    let a = app();
    let job = a.run_job(
        "annotate_save",
        json!({ "input": input, "annots": annots(60) }),
    );
    let out = done(&job, "three-pages-annotated.pdf");
    let want = [
        ("/Highlight".to_string(), true),
        ("/Square".to_string(), true),
    ];
    assert_eq!(written(&out), want);
    assert!(
        inked(Path::new(&input), &out, (90, 165)),
        "no highlight ink"
    );
    assert!(inked(Path::new(&input), &out, (60, 65)), "no rectangle ink");
    let job = a.run_job(
        "annotate_save",
        json!({ "input": input, "annots": annots(150) }),
    );
    assert_eq!(failed(&job), OUTSIDE);
    assert_eq!(
        names(d.path()),
        ["three-pages-annotated.pdf", "three-pages.pdf"]
    );
}

#[test]
fn annotate_over_mcp_writes_visible_annotations() {
    require_env!();
    let d = tempfile::tempdir().unwrap();
    let input = copy_input(d.path(), "three-pages.pdf");
    let s = serve(&d.path().join("cfg"), Registry::new(), noop());
    let m = Mcp::of(&s);
    let out = d.path().join("a.pdf");
    let args = json!({ "input": input, "annots": annots(60), "out": out });
    done(&m.run_job("pdf_annotate", args), "a.pdf");
    assert_eq!(written(&out).len(), 2);
    assert!(
        inked(Path::new(&input), &out, (90, 165)),
        "no highlight ink"
    );
    let bad = json!({ "input": input, "annots": annots(150) });
    assert_eq!(failed(&m.run_job("pdf_annotate", bad)), OUTSIDE);
    let e = m.call(
        "pdf_annotate",
        json!({ "input": input, "annots": [
        { "page": 0, "annot": { "type": "squiggly" } } ] }),
    );
    assert!(
        e.unwrap_err()
            .starts_with("invalid arguments: unknown variant `squiggly`")
    );
    let tools = m.rpc("tools/list", json!({}))["tools"].clone();
    let t = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "pdf_annotate");
    let about = t.unwrap()["description"].as_str().unwrap();
    assert!(about.starts_with("Adds annotations to a PDF"), "{about}");
    stop(s);
    assert_eq!(names(d.path()), ["a.pdf", "cfg", "three-pages.pdf"]);
}
