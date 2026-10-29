//! MCP server e2e over real HTTP on 127.0.0.1: auth, hardening, `mcp.json`,
//! handshake and tool listing. Tool behaviour lives in `mcp_tools.rs`.
mod common;
use common::mcp::{Mcp, noop, serve, stop};
use monax_core::Registry;
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

const TOOLS: [&str; 47] = [
    "pdf_capabilities",
    "pdf_job_status",
    "pdf_job_cancel",
    "pdf_open_in_app",
    "pdf_page_info",
    "pdf_read_text",
    "pdf_render_page",
    "pdf_extract",
    "pdf_delete",
    "pdf_rotate",
    "pdf_reorder",
    "pdf_organize",
    "pdf_merge",
    "pdf_alternate_mix",
    "pdf_split_pages",
    "pdf_split_size",
    "pdf_split_text",
    "pdf_split_bookmarks",
    "pdf_nup",
    "pdf_crop",
    "pdf_insert_blank",
    "pdf_split_half",
    "pdf_extract_images",
    "pdf_page_numbers",
    "pdf_header_footer",
    "pdf_watermark",
    "pdf_bates",
    "pdf_compress",
    "pdf_set_metadata",
    "pdf_flatten",
    "pdf_delete_annotations",
    "pdf_repair",
    "pdf_linearize",
    "pdf_encrypt",
    "pdf_decrypt",
    "pdf_unlock",
    "pdf_inspect_security",
    "pdf_redact",
    "pdf_ocr",
    "pdf_list_fields",
    "pdf_fill_forms",
    "pdf_sign_image",
    "pdf_sign_text",
    "pdf_annotate",
    "pdf_images_to_pdf",
    "pdf_to_images",
    "pdf_office_to_pdf",
];

fn list() -> Value {
    json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {} })
}

fn mode(p: &Path) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o777
}

#[test]
fn requests_without_the_right_token_get_401_and_no_detail() {
    let d = tempfile::tempdir().unwrap();
    let s = serve(d.path(), Registry::new(), noop());
    let m = Mcp::of(&s);
    let wrong = format!("Bearer {}", "0".repeat(64));
    let short = format!("Bearer {}", &m.token[..63]);
    for auth in ["", wrong.as_str(), short.as_str(), m.token.as_str()] {
        assert_eq!(
            m.raw(&[("Authorization", auth)], &list()),
            (401, String::new()),
            "{auth:?}"
        );
    }
    assert_eq!(m.raw(&[], &list()).0, 200);
    stop(s);
}

#[test]
fn foreign_host_or_origin_gets_403() {
    let d = tempfile::tempdir().unwrap();
    let s = serve(d.path(), Registry::new(), noop());
    let m = Mcp::of(&s);
    let local = format!("localhost:{}", m.port);
    let wrong_port = format!("127.0.0.1:{}", m.port.wrapping_add(1));
    assert_eq!(m.raw(&[("Host", &local)], &list()).0, 200);
    for host in ["evil.example", "evil.example:80", wrong_port.as_str()] {
        assert_eq!(m.raw(&[("Host", host)], &list()).0, 403, "{host}");
    }
    let own = format!("http://127.0.0.1:{}", m.port);
    assert_eq!(m.raw(&[("Origin", &own)], &list()).0, 200);
    for origin in ["https://evil.example", "http://localhost:1", "null"] {
        assert_eq!(m.raw(&[("Origin", origin)], &list()).0, 403, "{origin}");
    }
    stop(s);
}

#[test]
fn oversized_bodies_are_refused() {
    let d = tempfile::tempdir().unwrap();
    let s = serve(d.path(), Registry::new(), noop());
    let m = Mcp::of(&s);
    let big = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": { "x": "a".repeat(2 << 20) } });
    assert_eq!(m.raw(&[], &big).0, 413);
    stop(s);
}

#[test]
fn mcp_json_is_private_and_reused_across_restarts() {
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().join("monax");
    let s = serve(&dir, Registry::new(), noop());
    let first = s.info.clone();
    assert!(first.url.starts_with("http://127.0.0.1:") && first.url.ends_with("/mcp"));
    assert_eq!(first.token.expose().len(), 64);
    assert!(
        !format!("{first:?}").contains(first.token.expose()),
        "token in Debug"
    );
    let file = dir.join("mcp.json");
    let stored: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(stored, json!({ "url": first.url, "token": first.token }));
    assert_eq!((mode(&dir), mode(&file)), (0o700, 0o600));
    stop(s);

    let s = serve(&dir, Registry::new(), noop());
    assert_eq!(s.info, first, "same port and token after restart");
    assert!(!s.info.port_changed);
    // Port taken by the running server: the next one moves and rotates the
    // token, so the old bearer is useless to whoever holds the old port.
    let t = serve(&dir, Registry::new(), noop());
    assert_ne!(t.info.url, first.url);
    assert_ne!(t.info.token, first.token);
    assert!(t.info.port_changed);
    let stored: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(stored, json!({ "url": t.info.url, "token": t.info.token }));
    assert_eq!((mode(&dir), mode(&file)), (0o700, 0o600));
    let moved = t.info.clone();
    stop(t);
    stop(s);
    let s = serve(&dir, Registry::new(), noop());
    assert_eq!((&s.info.url, &s.info.token), (&moved.url, &moved.token));
    assert!(
        !s.info.port_changed,
        "free stored port: same port and token"
    );
    stop(s);

    std::fs::write(&file, "corrupt").unwrap();
    let s = serve(&dir, Registry::new(), noop());
    assert_ne!(
        s.info.token, first.token,
        "corrupt file regenerates the token"
    );
    assert_eq!(mode(&file), 0o600);
    stop(s);
}

#[test]
fn initialize_and_tools_list_expose_every_tool_with_a_schema() {
    let d = tempfile::tempdir().unwrap();
    let s = serve(d.path(), Registry::new(), noop());
    let m = Mcp::of(&s);
    let init = m.rpc(
        "initialize",
        json!({ "protocolVersion": "2025-06-18", "capabilities": {},
                "clientInfo": { "name": "test", "version": "1" } }),
    );
    assert_eq!(init["serverInfo"]["name"], "monax");
    assert!(init["capabilities"]["tools"].is_object(), "{init}");
    let tools = m.rpc("tools/list", json!({}))["tools"].clone();
    let all = tools.as_array().unwrap();
    let names: Vec<&str> = all.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, TOOLS);
    for t in all {
        assert_eq!(t["inputSchema"]["type"], "object", "{t}");
        assert!(!t["description"].as_str().unwrap().is_empty());
    }
    let text = &tools[5]["inputSchema"]["properties"];
    assert_eq!(
        text["input"]["description"],
        "Absolute path of the PDF to read."
    );
    assert!(
        text["pages"].is_object() && text["out"].is_object(),
        "{text}"
    );
    stop(s);
}
