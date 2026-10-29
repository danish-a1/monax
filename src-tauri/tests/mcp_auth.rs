//! MCP auth on the verbs `mcp.rs` does not post: GET and DELETE.
mod common;
use common::mcp::{Mcp, noop, serve, stop};
use monax_core::Registry;

#[test]
fn get_and_delete_without_the_right_token_get_401() {
    let d = tempfile::tempdir().unwrap();
    let s = serve(d.path(), Registry::new(), noop());
    let m = Mcp::of(&s);
    let wrong = format!("Bearer {}", "0".repeat(64));
    for method in ["GET", "DELETE"] {
        for auth in ["", wrong.as_str()] {
            let got = m.request(method, &[("Authorization", auth)], "");
            assert_eq!(got, (401, String::new()), "{method} {auth:?}");
        }
    }
    stop(s);
}
