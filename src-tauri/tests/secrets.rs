//! E2E: passwords at the boundary. The viewer commands open a protected PDF
//! with its password; argument errors over IPC and MCP never echo a secret.
mod common;
use common::mcp::{Mcp, noop, serve, stop};
use common::*;
use monax_core::Registry;
use serde_json::json;

#[test]
fn viewer_commands_take_a_password() {
    require_env!();
    let a = app();
    let enc = fixture("encrypted.pdf");
    let r = a.call("pages_info", json!({ "path": enc, "password": "user" }));
    assert_eq!(r.unwrap()["pages"].as_array().map(Vec::len), Some(3));
    let r = a.call("pages_info", json!({ "path": enc }));
    assert_eq!(r, Err(json!("encrypted input: a password is required")));
    let r = a.call("pages_info", json!({ "path": enc, "password": "x-Sesame" }));
    assert_eq!(r, Err(json!("wrong password")));
    let args = json!({ "path": enc, "page": 0, "max_px": 64, "password": "user" });
    assert_eq!(
        &a.call_raw("pages_thumbnail", args).unwrap()[..4],
        b"\x89PNG"
    );
    let args = json!({ "path": enc, "page": 0, "max_px": 64, "password": "x-Sesame" });
    assert_eq!(
        a.call("pages_thumbnail", args),
        Err(json!("wrong password"))
    );
}

#[test]
fn argument_errors_never_echo_secrets() {
    let a = app();
    let bad = [
        (
            "pages_merge",
            json!({ "input": { "inputs": ["/a.pdf"], "passwords": "x-Sesame" } }),
        ),
        (
            "pages_rotate",
            json!({ "input": { "input": "/a.pdf", "pages": [0], "degrees": 90, "password": ["x-Sesame"] } }),
        ),
        (
            "pages_info",
            json!({ "path": "/a.pdf", "password": { "k": "x-Sesame" } }),
        ),
    ];
    for (cmd, args) in bad {
        let e = a.call(cmd, args).unwrap_err().to_string();
        assert!(!e.contains("Sesame"), "{cmd}: secret echoed");
        assert!(e.contains("password"), "{cmd}: {e}");
    }
    let cfg = tempfile::tempdir().unwrap();
    let s = serve(cfg.path(), Registry::new(), noop());
    let m = Mcp::of(&s);
    let bad = [
        (
            "pdf_merge",
            json!({ "inputs": ["/a.pdf"], "passwords": "x-Sesame" }),
        ),
        (
            "pdf_decrypt",
            json!({ "input": "/a.pdf", "password": ["x-Sesame"] }),
        ),
        (
            "pdf_encrypt",
            json!({ "input": "/a.pdf", "opts": { "owner_pw": { "x": "x-Sesame" } } }),
        ),
    ];
    for (tool, args) in bad {
        let e = m.call(tool, args).unwrap_err();
        assert!(!e.contains("Sesame"), "{tool}: secret echoed");
        assert!(
            e.starts_with("invalid arguments: invalid password"),
            "{tool}: {e}"
        );
    }
    stop(s);
}
