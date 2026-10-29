//! Least privilege: the webview may listen for drag-drop and open files, nothing else.

#[test]
fn capability_grants_are_exactly_these() {
    let caps: serde_json::Value =
        serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
    assert_eq!(
        caps["permissions"],
        serde_json::json!([
            "core:event:allow-listen",
            "core:event:allow-unlisten",
            "dialog:allow-open"
        ])
    );
}
