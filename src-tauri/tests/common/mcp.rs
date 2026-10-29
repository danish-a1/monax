//! Raw HTTP/1.1 MCP client over `std::net::TcpStream` (no client crate) and a
//! headless server launcher with a temp-dir `mcp.json`.
use monax_core::Registry;
use monax_lib::mcp::{OpenHook, Server, start};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::{Duration, Instant};

pub fn serve(dir: &Path, reg: Registry, open: OpenHook) -> Server {
    tauri::async_runtime::block_on(start(dir, reg, open)).unwrap()
}

pub fn stop(s: Server) {
    tauri::async_runtime::block_on(s.stop());
}

pub fn noop() -> OpenHook {
    std::sync::Arc::new(|_| {})
}

/// Connection to a running server.
pub struct Mcp {
    pub port: u16,
    pub token: String,
}

impl Mcp {
    pub fn of(s: &Server) -> Self {
        Self::new(&s.info.url, s.info.token.expose())
    }

    /// From `http://127.0.0.1:<port>/mcp` and the token.
    pub fn new(url: &str, token: &str) -> Self {
        let port = url.rsplit(':').next().unwrap();
        let port = port.trim_end_matches("/mcp").parse().unwrap();
        let token = token.into();
        Self { port, token }
    }

    /// POSTs `body` to `/mcp`; `headers` override the defaults (Host,
    /// Authorization). Returns status and body.
    pub fn raw(&self, headers: &[(&str, &str)], body: &Value) -> (u16, String) {
        self.request("POST", headers, &body.to_string())
    }

    /// As [`Mcp::raw`] with any HTTP `method` and a raw `body`.
    pub fn request(&self, method: &str, headers: &[(&str, &str)], body: &str) -> (u16, String) {
        let auth = format!("Bearer {}", self.token);
        let host = format!("127.0.0.1:{}", self.port);
        let mut h: Vec<(&str, &str)> = vec![("Host", &host), ("Authorization", &auth)];
        for (k, v) in headers {
            h.retain(|(d, _)| !d.eq_ignore_ascii_case(k));
            h.push((k, v));
        }
        let mut req = format!("{method} /mcp HTTP/1.1\r\nContent-Type: application/json\r\n");
        req += "Accept: application/json, text/event-stream\r\nConnection: close\r\n";
        for (k, v) in h.iter().filter(|(_, v)| !v.is_empty()) {
            req += &format!("{k}: {v}\r\n");
        }
        req += &format!("Content-Length: {}\r\n\r\n{body}", body.len());
        let mut s = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
        // Write on another thread: a server refusing a large body answers and
        // closes before reading it all, which must not lose its response.
        let mut w = s.try_clone().unwrap();
        let writer = std::thread::spawn(move || w.write_all(req.as_bytes()));
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        while let Ok(n @ 1..) = s.read(&mut chunk) {
            buf.extend_from_slice(&chunk[..n]);
        }
        let _ = writer.join();
        let resp = String::from_utf8(buf).unwrap();
        let (head, body) = resp.split_once("\r\n\r\n").unwrap();
        let status = head[9..12].parse().unwrap();
        let chunked = head
            .to_ascii_lowercase()
            .contains("transfer-encoding: chunked");
        (status, if chunked { dechunk(body) } else { body.into() })
    }

    /// JSON-RPC `method`; returns `result` (panics on HTTP or RPC errors).
    pub fn rpc(&self, method: &str, params: Value) -> Value {
        let msg = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
        let (status, body) = self.raw(&[], &msg);
        assert_eq!(status, 200, "{body}");
        let v: Value = serde_json::from_str(&body).unwrap();
        v.get("result").cloned().unwrap_or_else(|| panic!("{v}"))
    }

    /// `tools/call`: `Ok(structuredContent)` or `Err(error text)`.
    pub fn call(&self, tool: &str, args: Value) -> Result<Value, String> {
        let r = self.rpc("tools/call", json!({ "name": tool, "arguments": args }));
        if r["isError"] == true {
            return Err(r["content"][0]["text"].as_str().unwrap().into());
        }
        Ok(r["structuredContent"].clone())
    }

    /// Calls a job tool and polls `pdf_job_status` until terminal.
    pub fn run_job(&self, tool: &str, args: Value) -> Value {
        let id = self.call(tool, args).unwrap()["job_id"].clone();
        let end = Instant::now() + Duration::from_secs(30);
        loop {
            let job = self
                .call("pdf_job_status", json!({ "job_id": id }))
                .unwrap();
            if !matches!(job["state"]["kind"].as_str(), Some("queued" | "running")) {
                return job;
            }
            assert!(Instant::now() < end, "job did not finish: {job}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

fn dechunk(mut s: &str) -> String {
    let mut out = String::new();
    while let Some((len, rest)) = s.split_once("\r\n") {
        let n = usize::from_str_radix(len.trim(), 16).unwrap_or(0);
        if n == 0 {
            break;
        }
        out += &rest[..n];
        s = &rest[n + 2..];
    }
    out
}
