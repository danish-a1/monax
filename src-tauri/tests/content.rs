//! E2E: content commands over IPC through the job registry.
mod common;
use common::*;
use serde_json::{Value, json};
use std::path::Path;

fn text(out: &Value, page: usize) -> String {
    monax_core::pdfium::extract_text(Path::new(out.as_str().unwrap()), page..=page).unwrap()
}

fn done(job: &Value) -> &Value {
    assert_eq!(job["state"]["kind"], "done", "{job}");
    &job["outputs"][0]
}

#[test]
fn content_commands_write_stamped_copies() {
    if !have_pdfium() {
        return;
    }
    let a = app();
    let d = tempfile::tempdir().unwrap();
    let input = copy_input(d.path(), "three-pages.pdf");
    let j = a.run_job(
        "content_page_numbers",
        json!({ "input": input, "opts": { "format": "%[page]/%[pages]", "pages": [1] } }),
    );
    let out = done(&j);
    assert!(
        out.as_str()
            .unwrap()
            .ends_with("three-pages-page-numbers.pdf")
    );
    assert!(text(out, 1).contains("1/3"));
    let hf = json!({ "header": { "right": "Top" }, "footer": { "left": "Bottom" } });
    let j = a.run_job(
        "content_header_footer",
        json!({ "input": input, "opts": hf }),
    );
    assert!(text(done(&j), 2).contains("Top"));
    let j = a.run_job(
        "content_watermark",
        json!({ "input": input, "opts": { "text": "SECRET", "color": [200, 0, 0] } }),
    );
    assert!(text(done(&j), 0).contains("SECRET"));
    let out = d.path().join("bates.pdf");
    let j = a.run_job(
        "content_bates",
        json!({ "input": input, "opts": { "prefix": "X", "digits": 3, "start": 41 }, "out": out }),
    );
    assert_eq!(done(&j).as_str().unwrap(), out.to_str().unwrap());
    assert!(text(done(&j), 2).contains("X043"));
}

#[test]
fn content_failures_surface_the_core_message() {
    if !have_pdfium() {
        return;
    }
    let a = app();
    let d = tempfile::tempdir().unwrap();
    let input = copy_input(d.path(), "three-pages.pdf");
    let enc = copy_input(d.path(), "encrypted.pdf");
    let cases = [
        (
            "content_watermark",
            json!({ "input": input, "opts": { "text": "\u{3b1}" } }),
            "invalid input: character '\u{3b1}' (U+03B1) is not supported by Helvetica (WinAnsi)",
        ),
        (
            "content_header_footer",
            json!({ "input": input }),
            "invalid input: header and footer are all empty",
        ),
        (
            "content_bates",
            json!({ "input": input, "opts": { "digits": 0 } }),
            "invalid input: digits must be 1..=12, got 0",
        ),
        (
            "content_page_numbers",
            json!({ "input": input, "opts": { "pages": [5] } }),
            "page index 5 out of range (document has 3 pages, indices 0-2)",
        ),
        (
            "content_page_numbers",
            json!({ "input": enc }),
            "encrypted input: a password is required",
        ),
    ];
    for (cmd, args, want) in cases {
        let job = a.run_job(cmd, args);
        assert_eq!(job["state"]["kind"], "failed", "{job}");
        assert_eq!(job["state"]["message"], want, "{cmd}");
    }
    let names: Vec<_> = std::fs::read_dir(d.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 2, "no output on failure");
}
