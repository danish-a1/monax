//! E2E: every `pdf_<op>` tool over real HTTP with the token: submit, poll
//! `pdf_job_status`, check the output files on disk or the exact error.
mod common;
use common::mcp::{Mcp, noop, serve, stop};
use common::ops::{check_success, failures, noted, successes};
use monax_core::Registry;
use serde_json::{Value, json};

#[test]
fn every_op_tool_writes_its_outputs_and_reports_failures() {
    require_env!();
    let cfg = tempfile::tempdir().unwrap();
    let s = serve(cfg.path(), Registry::new(), noop());
    let m = Mcp::of(&s);
    let d = tempfile::tempdir().unwrap();
    for c in successes(d.path()) {
        let job = m.run_job(&format!("pdf_{}", c.op), c.args.clone());
        check_success(&c, &job);
    }
    let d = tempfile::tempdir().unwrap();
    let cases = failures(d.path());
    let before = std::fs::read_dir(d.path()).unwrap().count();
    for (op, _, args, want) in cases {
        let job = m.run_job(&format!("pdf_{op}"), args);
        assert_eq!(job["state"]["kind"], "failed", "pdf_{op}: {job}");
        assert!(
            !job.to_string().contains("Sesame"),
            "pdf_{op}: secret echoed"
        );
        let got = (job["state"]["message"].as_str(), &job["outputs"]);
        assert_eq!(got, (Some(want), &json!([])), "pdf_{op}");
    }
    assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), before);
    let (op, _, args, notes) = noted(d.path());
    let job = m.run_job(&format!("pdf_{op}"), args);
    assert_eq!(job["notes"], notes, "{job}");
    let e = m.call("pdf_split_text", json!({ "input": "/a.pdf" }));
    assert_eq!(e, Err("invalid arguments: missing field `text`".into()));
    stop(s);
}

#[test]
fn op_descriptions_and_field_docs_come_from_core() {
    let cfg = tempfile::tempdir().unwrap();
    let s = serve(cfg.path(), Registry::new(), noop());
    let tools = Mcp::of(&s).rpc("tools/list", json!({}))["tools"].clone();
    let tool = |name: &str| -> Value {
        let all = tools.as_array().unwrap();
        all.iter().find(|t| t["name"] == name).unwrap().clone()
    };
    let crop = tool("pdf_crop");
    let about = crop["description"].as_str().unwrap();
    assert!(
        about.starts_with("Sets the CropBox of the chosen pages to `rect`"),
        "{about}"
    );
    assert!(about.contains("origin bottom-left, y-up"), "{about}");
    assert!(
        about.ends_with("failed (state.message is the reason) or cancelled."),
        "{about}"
    );
    let props = &crop["inputSchema"]["properties"];
    assert!(
        props["pages"]["description"]
            .as_str()
            .unwrap()
            .starts_with("0-based")
    );
    let split = tool("pdf_split_pages")["inputSchema"].clone();
    assert_eq!(split["required"], json!(["input", "ranges"]), "{split}");
    let pattern = split["properties"]["pattern"]["description"]
        .as_str()
        .unwrap();
    assert!(pattern.contains("`%[filename]-%[index].pdf`"), "{pattern}");
    stop(s);
}
