//! E2E: form fields listed and filled over IPC and over MCP through the
//! job registry; the filled copy reads back its values.
mod common;
use common::mcp::{Mcp, noop, serve, stop};
use common::*;
use monax_core::Registry;
use monax_core::forms::list_fields;
use serde_json::{Value, json};
use std::path::Path;

const UNKNOWN: &str = "invalid input: unknown field \"nope\"; valid fields: person.name, \
    email, agree, color, size, fruit, go, sig, locked, note";

/// The value `list_fields` reads for `name` in the done job's one output,
/// which ends with `file`.
fn value(job: &Value, file: &str, name: &str) -> String {
    assert_eq!(job["state"]["kind"], "done", "{job}");
    let out = Path::new(job["outputs"][0].as_str().unwrap());
    assert!(out.ends_with(file), "{out:?}");
    let all = list_fields(out).unwrap();
    all.into_iter().find(|f| f.name == name).unwrap().value
}

fn failed(job: &Value) -> &str {
    assert_eq!(job["state"]["kind"], "failed", "{job}");
    assert_eq!(job["outputs"], json!([]));
    job["state"]["message"].as_str().unwrap()
}

#[test]
fn forms_over_ipc_list_and_fill() {
    require_env!();
    let d = tempfile::tempdir().unwrap();
    let input = copy_input(d.path(), "form.pdf");
    let a = app();
    let fields = a
        .call("forms_list_fields", json!({ "path": input }))
        .unwrap();
    assert_eq!(fields.as_array().unwrap().len(), 10, "{fields}");
    assert_eq!(fields[2]["kind"], "checkbox");
    assert_eq!(fields[2]["options"], json!(["Agreed"]));
    let e = a.call("forms_list_fields", json!({ "path": "rel.pdf" }));
    assert_eq!(
        e,
        Err(json!("invalid input: path must be absolute: rel.pdf"))
    );
    let values = json!([{ "name": "agree", "value": "true" }, { "name": "email", "value": "a@b" }]);
    let job = a.run_job("forms_fill", json!({ "input": input, "values": values }));
    assert_eq!(value(&job, "form-filled.pdf", "agree"), "Agreed");
    let bad = json!([{ "name": "nope", "value": "x" }]);
    assert_eq!(
        failed(&a.run_job("forms_fill", json!({ "input": input, "values": bad }))),
        UNKNOWN
    );
    let opts = json!({ "user_pw": "u", "owner_pw": "o" });
    let enc = a.run_job("security_encrypt", json!({ "input": input, "opts": opts }));
    let enc = enc["outputs"][0].clone();
    let e = a.call("forms_list_fields", json!({ "path": enc }));
    assert_eq!(e, Err(json!("encrypted input: a password is required")));
    let f = a.call("forms_list_fields", json!({ "path": enc, "password": "u" }));
    assert_eq!(f.unwrap(), fields);
    assert_eq!(
        names(d.path()),
        ["form-encrypt.pdf", "form-filled.pdf", "form.pdf"]
    );
}

#[test]
fn forms_over_mcp_write_a_fields_json_and_a_filled_copy() {
    require_env!();
    let d = tempfile::tempdir().unwrap();
    let input = copy_input(d.path(), "form.pdf");
    let s = serve(&d.path().join("cfg"), Registry::new(), noop());
    let m = Mcp::of(&s);
    let job = m.run_job("pdf_list_fields", json!({ "input": input }));
    assert_eq!(job["state"]["kind"], "done", "{job}");
    let out = job["outputs"][0].as_str().unwrap();
    assert_eq!(Path::new(out), d.path().join("form-fields.json"));
    let v: Value = serde_json::from_slice(&std::fs::read(out).unwrap()).unwrap();
    assert_eq!(v["fields"][0]["name"], "person.name");
    assert_eq!(v["fields"][0]["value"], "Ann");
    let values = json!([{ "name": "person.name", "value": "Zed" }]);
    let job = m.run_job(
        "pdf_fill_forms",
        json!({ "input": input, "values": values }),
    );
    assert_eq!(value(&job, "form-filled.pdf", "person.name"), "Zed");
    let bad = json!([{ "name": "nope", "value": "x" }]);
    assert_eq!(
        failed(&m.run_job("pdf_fill_forms", json!({ "input": input, "values": bad }))),
        UNKNOWN
    );
    let e = m.call("pdf_fill_forms", json!({ "input": input }));
    assert!(
        e.unwrap_err()
            .starts_with("invalid arguments: missing field `values`")
    );
    stop(s);
    assert_eq!(
        names(d.path()),
        ["cfg", "form-fields.json", "form-filled.pdf", "form.pdf"]
    );
}
