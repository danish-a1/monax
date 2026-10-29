//! E2E: images to PDF, PDF to images and PDF to text over IPC and over MCP
//! through the job registry; each job's outputs are the files on disk.
mod common;
use common::mcp::{Mcp, noop, serve, stop};
use common::*;
use monax_core::Registry;
use monax_core::pdfium::displayed_size;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// The done job's outputs; each exists.
fn outputs(job: &Value) -> Vec<PathBuf> {
    assert_eq!(job["state"]["kind"], "done", "{job}");
    let v = job["outputs"].as_array().unwrap();
    let v: Vec<PathBuf> = v.iter().map(|p| p.as_str().unwrap().into()).collect();
    assert!(v.iter().all(|p| p.is_file()), "{v:?}");
    v
}

fn failed(job: Value) -> String {
    assert_eq!(job["state"]["kind"], "failed", "{job}");
    assert_eq!(job["outputs"], json!([]));
    job["state"]["message"].as_str().unwrap().into()
}

fn ends(paths: &[PathBuf]) -> Vec<String> {
    let name = |p: &PathBuf| p.file_name().unwrap().to_string_lossy().into_owned();
    paths.iter().map(name).collect()
}

#[test]
fn convert_over_ipc() {
    require_env!();
    let d = tempfile::tempdir().unwrap();
    let pdf = copy_input(d.path(), "three-pages.pdf");
    let png = copy_input(d.path(), "signature.png");
    let a = app();
    let size = json!({ "width": 595.28, "height": 841.89 });
    let job = a.run_job(
        "convert_images_to_pdf",
        json!({ "inputs": [png, png], "page_size": size }),
    );
    let out = outputs(&job);
    assert_eq!(ends(&out), ["signature-images.pdf"]);
    assert_eq!(pages(&json!(out[0])), 2);
    let (w, h) = displayed_size(&out[0], 1).unwrap();
    assert!((w - 595.28).abs() < 0.01 && (h - 841.89).abs() < 0.01);
    let job = a.run_job(
        "convert_pdf_to_images",
        json!({ "input": pdf, "format": "jpeg", "pages": [2, 0] }),
    );
    assert_eq!(
        ends(&outputs(&job)),
        ["three-pages-3.jpg", "three-pages-1.jpg"]
    );
    let job = a.run_job("read_text", json!({ "input": pdf, "pages": [1] }));
    let txt = &outputs(&job)[0];
    assert!(std::fs::read_to_string(txt).unwrap().contains("two"));
    let bad = json!({ "input": pdf, "dpi": 700 });
    let m = failed(a.run_job("convert_pdf_to_images", bad));
    assert_eq!(m, "invalid input: dpi 700 not in 72..=600");
    let m = failed(a.run_job("convert_images_to_pdf", json!({ "inputs": [pdf] })));
    assert_eq!(
        m,
        format!("invalid input: image {pdf} is not a PNG or JPEG")
    );
    assert_eq!(
        names(d.path()),
        [
            "signature-images.pdf",
            "signature.png",
            "three-pages-1.jpg",
            "three-pages-3.jpg",
            "three-pages-read-text.txt",
            "three-pages.pdf"
        ]
    );
}

#[test]
fn convert_over_mcp() {
    require_env!();
    let d = tempfile::tempdir().unwrap();
    let pdf = copy_input(d.path(), "three-pages.pdf");
    let png = copy_input(d.path(), "signature.png");
    let s = serve(&d.path().join("cfg"), Registry::new(), noop());
    let m = Mcp::of(&s);
    let out = d.path().join("i.pdf");
    let job = m.run_job("pdf_images_to_pdf", json!({ "inputs": [png], "out": out }));
    assert_eq!(outputs(&job), [out]);
    let job = m.run_job("pdf_to_images", json!({ "input": pdf, "dpi": 72 }));
    let pngs = outputs(&job);
    assert_eq!(
        ends(&pngs),
        [
            "three-pages-1.png",
            "three-pages-2.png",
            "three-pages-3.png"
        ]
    );
    assert!(std::fs::read(&pngs[0]).unwrap().starts_with(b"\x89PNG"));
    let bad = json!({ "input": pdf, "pages": [] });
    assert_eq!(
        failed(m.run_job("pdf_to_images", bad)),
        "invalid input: no pages given"
    );
    let e = m.call("pdf_to_images", json!({ "input": pdf, "format": "gif" }));
    assert!(
        e.unwrap_err()
            .starts_with("invalid arguments: unknown variant `gif`")
    );
    stop(s);
    let mut want = vec!["cfg", "i.pdf", "signature.png"];
    want.extend([
        "three-pages-1.png",
        "three-pages-2.png",
        "three-pages-3.png",
    ]);
    want.push("three-pages.pdf");
    assert_eq!(names(d.path()), want);
    assert!(Path::new(&pdf).is_file());
}
