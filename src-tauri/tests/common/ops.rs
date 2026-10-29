//! One success case and one failure case per core op, shared by the IPC and
//! the MCP e2e tests: both boundaries take the same core input JSON.
use super::copy_input;
use serde_json::{Value, json};
use std::path::Path;

/// `op` is the core fn name: Tauri command `cmd`, MCP tool `pdf_<op>`.
pub struct Case {
    pub op: &'static str,
    pub cmd: &'static str,
    pub args: Value,
    /// Page count of each output in order; 0 for a non-PDF output (PNG).
    pub pages: Vec<usize>,
}

fn case(op: &'static str, cmd: &'static str, args: Value, pages: &[usize]) -> Case {
    let pages = pages.to_vec();
    Case {
        op,
        cmd,
        args,
        pages,
    }
}

/// Every op with default output naming, inputs copied into `dir`; the
/// last rows open encrypted inputs with a password.
#[rustfmt::skip]
pub fn successes(dir: &Path) -> Vec<Case> {
    let p = copy_input(dir, "three-pages.pdf");
    let o = copy_input(dir, "outlines.pdf");
    let img = copy_input(dir, "image.pdf");
    let an = copy_input(dir, "annots.pdf");
    let enc = copy_input(dir, "encrypted.pdf");
    let own = copy_input(dir, "owner-only.pdf");
    let lock = json!({ "owner_pw": "o-Sesame", "permissions": { "print": "none" } });
    let rect = json!({ "left": 10, "bottom": 10, "right": 100, "top": 100 });
    let ranges = json!([{ "start": 0, "end": 0 }, { "start": 1, "end": 2 }]);
    vec![
        case("extract", "pages_extract", json!({ "input": p, "pages": [0, 2] }), &[2]),
        case("delete", "pages_delete", json!({ "input": p, "pages": [1] }), &[2]),
        case("rotate", "pages_rotate", json!({ "input": p, "pages": [0], "degrees": 90 }), &[3]),
        case("reorder", "pages_reorder", json!({ "input": p, "order": [2, 1, 0] }), &[3]),
        case("organize", "pages_organize", json!({ "input": p, "plan": [{ "page": 2, "rotate": 90 }] }), &[1]),
        case("merge", "pages_merge", json!({ "inputs": [p, p] }), &[6]),
        case("alternate_mix", "pages_alternate_mix", json!({ "a": p, "b": o, "reverse_b": true }), &[6]),
        case("split_pages", "pages_split_pages", json!({ "input": p, "ranges": ranges }), &[1, 2]),
        case("split_size", "pages_split_size", json!({ "input": p, "max_bytes": 1_000_000_000 }), &[3]),
        case("split_text", "pages_split_text", json!({ "input": p, "text": "two" }), &[1, 2]),
        case("split_bookmarks", "pages_split_bookmarks", json!({ "input": o }), &[2, 1]),
        case("nup", "pages_nup", json!({ "input": p, "per_sheet": 4 }), &[1]),
        case("crop", "pages_crop", json!({ "input": p, "pages": [0], "rect": rect }), &[3]),
        case("insert_blank", "pages_insert_blank", json!({ "input": p, "after": [1] }), &[4]),
        case("split_half", "pages_split_half", json!({ "input": p }), &[6]),
        case("extract_images", "pages_extract_images", json!({ "input": img }), &[0]),
        case("page_numbers", "content_page_numbers", json!({ "input": p }), &[3]),
        case("header_footer", "content_header_footer", json!({ "input": p, "opts": { "header": { "center": "T" } } }), &[3]),
        case("watermark", "content_watermark", json!({ "input": p, "opts": { "text": "W" } }), &[3]),
        case("bates", "content_bates", json!({ "input": p }), &[3]),
        case("compress", "content_compress", json!({ "input": img, "opts": { "dpi": 72 } }), &[1]),
        case("set_metadata", "content_set_metadata", json!({ "input": p, "metadata": { "title": "T" } }), &[3]),
        case("flatten", "content_flatten", json!({ "input": an }), &[2]),
        case("delete_annotations", "content_delete_annotations", json!({ "input": an, "pages": [0] }), &[2]),
        case("repair", "content_repair", json!({ "input": p }), &[3]),
        case("linearize", "content_linearize", json!({ "input": p }), &[3]),
        case("encrypt", "security_encrypt", json!({ "input": p, "opts": lock }), &[3]),
        case("decrypt", "security_decrypt", json!({ "input": enc, "password": "user" }), &[3]),
        case("unlock", "security_unlock", json!({ "input": own }), &[3]),
        case("inspect_security", "security_inspect", json!({ "input": enc }), &[0]),
        case("redact", "security_redact", json!({ "input": p, "boxes": [{ "page": 1, "rect": rect }] }), &[3]),
        case("rotate", "pages_rotate", json!({ "input": enc, "pages": [0], "degrees": 90, "password": "user" }), &[3]),
        case("merge", "pages_merge", json!({ "inputs": [p, enc], "passwords": [null, "user"] }), &[6]),
    ]
}

/// (op, command, args, exact job failure message) for each op added in 11b,
/// 5b, 6a and 6b; none of them may leave a file in `dir`. Passwords in these
/// cases contain "Sesame", which no job may echo.
#[rustfmt::skip]
pub fn failures(dir: &Path) -> Vec<(&'static str, &'static str, Value, &'static str)> {
    let p = copy_input(dir, "three-pages.pdf");
    let enc = copy_input(dir, "encrypted.pdf");
    let cut = dir.join("cut.pdf");
    std::fs::write(&cut, b"%PDF-1.4\n").unwrap();
    let bad_range = json!([{ "start": 2, "end": 1 }]);
    let unit = json!({ "left": 0, "bottom": 0, "right": 1, "top": 1 });
    vec![
        ("alternate_mix", "pages_alternate_mix", json!({ "a": p, "b": "rel.pdf" }), "invalid input: path must be absolute: rel.pdf"),
        ("split_pages", "pages_split_pages", json!({ "input": p, "ranges": bad_range }), "invalid input: range start 2 is after end 1"),
        ("split_size", "pages_split_size", json!({ "input": p, "max_bytes": 0 }), "invalid input: max size must be positive"),
        ("split_text", "pages_split_text", json!({ "input": p, "text": "" }), "invalid input: search text is empty"),
        ("split_bookmarks", "pages_split_bookmarks", json!({ "input": p }), "invalid input: no bookmarks at level 1"),
        ("nup", "pages_nup", json!({ "input": p, "per_sheet": 3 }), "invalid input: pages per sheet must be 2, 4, 6, 9 or 16, got 3"),
        ("crop", "pages_crop", json!({ "input": p, "pages": [5], "rect": unit }), "page index 5 out of range (document has 3 pages, indices 0-2)"),
        ("insert_blank", "pages_insert_blank", json!({ "input": p, "after": [] }), "invalid input: no pages given"),
        ("split_half", "pages_split_half", json!({ "input": enc }), "encrypted input: a password is required"),
        ("extract_images", "pages_extract_images", json!({ "input": p, "out_dir": "rel" }),
         "invalid input: output directory must be an absolute path to an existing directory: rel"),
        ("compress", "content_compress", json!({ "input": p, "opts": { "dpi": 0 } }), "invalid input: dpi must be in 1..=2400, got 0"),
        ("set_metadata", "content_set_metadata", json!({ "input": enc }), "encrypted input: a password is required"),
        ("flatten", "content_flatten", json!({ "input": "rel.pdf" }), "invalid input: path must be absolute: rel.pdf"),
        ("delete_annotations", "content_delete_annotations", json!({ "input": p, "pages": [5] }), "page index 5 out of range (document has 3 pages, indices 0-2)"),
        ("repair", "content_repair", json!({ "input": enc }), "encrypted input: a password is required"),
        ("linearize", "content_linearize", json!({ "input": cut }), "damaged file"),
        ("encrypt", "security_encrypt", json!({ "input": p, "opts": { "user_pw": "u-Sesame", "owner_pw": "" } }),
         "invalid input: owner password is required"),
        ("decrypt", "security_decrypt", json!({ "input": enc, "password": "w-Sesame" }), "wrong password"),
        ("unlock", "security_unlock", json!({ "input": enc }), "encrypted input: a password is required"),
        ("inspect_security", "security_inspect", json!({ "input": enc, "password": "w-Sesame" }), "wrong password"),
        ("rotate", "pages_rotate", json!({ "input": enc, "pages": [0], "degrees": 90, "password": "w-Sesame" }), "wrong password"),
        ("redact", "security_redact", json!({ "input": p, "boxes": [{ "page": 0, "rect": { "left": 300, "bottom": 0, "right": 400, "top": 50 } }] }),
         "invalid input: redaction rect on page 0 is outside the page"),
    ]
}

/// Panics unless `job` (a finished job of `c`) wrote outputs with the
/// expected page counts, notes an unprotected output when `c` opened an
/// input with a password (not decrypt: removing it is its purpose), and
/// echoes no password.
pub fn check_success(c: &Case, job: &Value) {
    assert_eq!(output_pages(job), c.pages, "{}: {job}", c.cmd);
    let secret = c.args.get("password").is_some() || c.args.get("passwords").is_some();
    let opened = secret && !c.cmd.starts_with("security_"); // decrypt takes its own
    let noted = job["notes"]
        .as_array()
        .is_some_and(|n| n.contains(&json!("output is not password-protected")));
    assert_eq!(noted, opened, "{}: {job}", c.cmd);
    assert!(
        !job.to_string().contains("Sesame"),
        "{}: secret echoed",
        c.cmd
    );
}

/// Page count of each output of a finished job (0 for a non-PDF file);
/// panics unless the job is done and every output exists.
pub fn output_pages(job: &Value) -> Vec<usize> {
    assert_eq!(job["state"]["kind"], "done", "{job}");
    let outs = job["outputs"].as_array().unwrap().iter();
    outs.map(|o| {
        let p = Path::new(o.as_str().unwrap());
        assert!(p.is_file(), "{job}");
        match p.extension().is_some_and(|e| e == "pdf") {
            true => monax_core::pdfium::page_count(p).unwrap(),
            false => 0,
        }
    })
    .collect()
}

/// compress on a CMYK image drawn above 72 dpi: (op, command, args, the
/// job's exact notes). The job is done and the image is left as it is.
pub fn noted(dir: &Path) -> (&'static str, &'static str, Value, Value) {
    let args = json!({ "input": copy_input(dir, "cmyk.pdf"), "opts": { "dpi": 72 } });
    let notes = json!(["1 image left above 72 dpi: color space not gray or RGB"]);
    ("compress", "content_compress", args, notes)
}
