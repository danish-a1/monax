mod common;
use common::guard::Op;
use monax_core::pages::{self, Rect};

const BOX: Rect = Rect {
    left: 0.,
    bottom: 0.,
    right: 100.,
    top: 100.,
};

/// The pdfium page edits, as `(input, out) -> result`.
const OPS: [(&str, Op); 4] = [
    ("nup", |i, o| pages::nup(i, 2, o)),
    ("crop", |i, o| pages::crop(i, &[0], BOX, o)),
    ("insert_blank", |i, o| pages::insert_blank(i, &[0], o)),
    ("split_half", pages::split_half),
];

guard_tests!(OPS, common::have_pdfium());
