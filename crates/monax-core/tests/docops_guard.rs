//! Copy-on-save and input guards of the document-level content ops.
mod common;
use common::guard::{self, Op};
use monax_core::content::{self, CompressOpts, Metadata};

const OPS: [(&str, Op); 6] = [
    ("compress", |i, o| {
        content::compress(i, o, CompressOpts::default())
    }),
    ("set_metadata", |i, o| {
        content::set_metadata(i, o, Metadata::default())
    }),
    ("flatten", content::flatten),
    ("delete_annotations", |i, o| {
        content::delete_annotations(i, o, None)
    }),
    ("linearize", content::linearize),
    ("repair", content::repair),
];

// repair is meant to read damaged input, so it skips that one case
// (`docops.rs` covers it).
const READS_DAMAGED: usize = 5;

fn ready() -> bool {
    common::have_pdfium() && common::have_qpdf()
}

#[test]
fn refuses_to_write_over_input() {
    if ready() {
        guard::refuses_to_write_over_input(&OPS);
    }
}

#[test]
fn never_overwrites_existing_output() {
    if ready() {
        guard::never_overwrites_existing_output(&OPS);
    }
}

#[test]
fn bad_out_paths_are_invalid() {
    if ready() {
        guard::bad_out_paths_are_invalid(&OPS);
    }
}

#[test]
fn encrypted_input_fails_without_output() {
    if ready() {
        guard::encrypted_input_fails_without_output(&OPS);
    }
}

#[test]
fn damaged_input_fails_without_output() {
    if ready() {
        guard::damaged_input_fails_without_output(&OPS[..READS_DAMAGED]);
    }
}
