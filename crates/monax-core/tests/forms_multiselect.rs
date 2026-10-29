//! A multi-select list box lists only its first selected value, and says so.
mod common;
use common::docs::objects;
use common::forms::{derive, field, form};
use monax_core::forms::ListFieldsInput;
use serde_json::json;

#[test]
fn a_multi_select_list_box_gets_a_note_naming_the_field_and_count() {
    require_qpdf!();
    let (d, p) = form();
    let (key, mut fruit) = objects(&p)
        .into_iter()
        .map(|(k, o)| (k, o["value"].clone()))
        .find(|(_, v)| v["/T"] == json!("u:fruit"))
        .unwrap();
    fruit["/V"] = json!(["u:apple", "u:pear"]);
    let multi = derive(&p, "multi.pdf", json!({ key: { "value": fruit } }));
    let input = json!({ "input": multi });
    let (_, notes) = serde_json::from_value::<ListFieldsInput>(input)
        .unwrap()
        .run()
        .unwrap();
    assert_eq!(
        notes,
        ["field \"fruit\" has 2 selected values; only the first is shown"]
    );
    assert_eq!(field(&multi, "fruit").value, "apple");
    let one = derive(&p, "one.pdf", json!({}));
    let (_, none) = serde_json::from_value::<ListFieldsInput>(json!({ "input": one }))
        .unwrap()
        .run()
        .unwrap();
    assert!(none.is_empty(), "{none:?}");
    drop(d);
}
