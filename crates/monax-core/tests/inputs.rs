use monax_core::pages::{ExtractInput, MergeInput, OrganizeInput};
use schemars::schema_for;
use serde_json::json;

fn required(schema: &schemars::Schema) -> Vec<String> {
    schema.as_value()["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn extract_input_round_trips_with_optional_out() {
    let v = json!({"input": "/a.pdf", "pages": [0, 2]});
    let i: ExtractInput = serde_json::from_value(v).unwrap();
    assert_eq!(i.pages, [0, 2]);
    assert!(i.out.is_none());
    let back = serde_json::to_value(&i).unwrap();
    assert_eq!(back["input"], "/a.pdf");
}

#[test]
fn schema_requires_the_essential_fields() {
    let r = required(&schema_for!(ExtractInput));
    assert!(r.contains(&"input".into()) && r.contains(&"pages".into()));
    assert!(!r.contains(&"out".into()));
    let r = required(&schema_for!(OrganizeInput));
    assert!(r.contains(&"plan".into()));
}

#[test]
fn merge_opts_default_when_omitted() {
    let i: MergeInput = serde_json::from_value(json!({"inputs": ["/a.pdf"]})).unwrap();
    assert!(!i.opts.blank_before_even);
}

#[test]
fn unknown_fields_are_rejected() {
    let v = json!({"input": "/a.pdf", "pages": [0], "bogus": 1});
    assert!(serde_json::from_value::<ExtractInput>(v).is_err());
}
