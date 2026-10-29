use monax_core::names::default_output;
use std::path::Path;

#[test]
fn sits_next_to_input_named_after_tool() {
    let got = default_output(Path::new("/docs/My Report.pdf"), "extract");
    assert_eq!(got, Path::new("/docs/My Report-extract.pdf"));
}

#[test]
fn separators_in_tool_id_cannot_escape_the_directory() {
    let got = default_output(Path::new("/docs/a.pdf"), "../x");
    assert_eq!(got.parent(), Some(Path::new("/docs")));
}

#[test]
fn does_not_touch_the_filesystem_or_pick_free_names() {
    let got = default_output(Path::new("/definitely/not/there/a.pdf"), "merge");
    assert_eq!(got, Path::new("/definitely/not/there/a-merge.pdf"));
}
