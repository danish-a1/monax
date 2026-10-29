//! Embedded MCP server: `rmcp` streamable HTTP on `127.0.0.1:<port>/mcp`,
//! bearer-token gated, sharing the GUI's job registry. Tools live in `tools`.
mod args;
mod config;
mod handler;
mod tools;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use monax_core::Registry;
use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use serde::Serialize;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::net::TcpListener;

/// Called by `pdf_open_in_app` with a validated absolute path.
pub type OpenHook = Arc<dyn Fn(&Path) + Send + Sync>;

/// Where an agent connects.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct McpInfo {
    pub url: String,
    /// Bearer token: a secret, shown only in Settings (serialized as is).
    pub token: monax_core::Secret,
    /// The stored port was taken, so the server moved and the token was
    /// rotated: the saved connection command must be re-run.
    pub port_changed: bool,
}

/// Server state shown in Settings. JSON:
/// `{"status":"running","url","token","port_changed"}`,
/// `{"status":"failed","reason"}` or `{"status":"starting"}`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum McpStatus {
    Starting,
    Running(McpInfo),
    Failed { reason: String },
}

/// Tauri state holding the status, set once at launch.
#[derive(Default)]
pub struct McpState(OnceLock<McpStatus>);

impl McpState {
    /// Records the launch outcome; later calls are ignored.
    pub fn set(&self, s: McpStatus) {
        let _ = self.0.set(s);
    }

    pub fn get(&self) -> McpStatus {
        self.0.get().cloned().unwrap_or(McpStatus::Starting)
    }
}

/// A running server; dropping it leaves it running, [`Server::stop`] ends it.
pub struct Server {
    pub info: McpInfo,
    task: tokio::task::JoinHandle<()>,
}

impl Server {
    /// Stops serving and waits until the port is released.
    pub async fn stop(self) {
        self.task.abort();
        let _ = self.task.await;
    }
}

/// Starts the server with the `mcp.json` in `dir`: reuses its port and token
/// when the port is free; if the port is taken, moves to a random port with a
/// new token (`port_changed`); a missing or corrupt file gets both fresh.
/// Then rewrites the file. Must run inside a tokio runtime.
///
/// # Errors
/// Binding, randomness or writing `mcp.json` failed.
pub async fn start(dir: &Path, reg: Registry, open: OpenHook) -> std::io::Result<Server> {
    let stored = config::load(dir);
    let reused = match &stored {
        Some((port, _)) => TcpListener::bind((Ipv4Addr::LOCALHOST, *port)).await.ok(),
        None => None,
    };
    let port_changed = stored.is_some() && reused.is_none();
    let listener = match reused {
        Some(l) => l,
        None => TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?,
    };
    let port = listener.local_addr()?.port();
    // Keep the token only with its port: after a move, the saved bearer must
    // not stay valid for whatever process now holds the old port.
    let token = match stored {
        Some((_, t)) if !port_changed => t,
        _ => config::new_token()?,
    };
    let token = monax_core::Secret::new(token);
    let info = McpInfo {
        url: config::url(port),
        token,
        port_changed,
    };
    config::save(dir, &info.url, info.token.expose())?;
    let local = |scheme: &str| {
        [
            format!("{scheme}127.0.0.1:{port}"),
            format!("{scheme}localhost:{port}"),
        ]
    };
    let cfg = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false) // stateless: no session state to leak or expire
        .with_json_response(true)
        .with_sse_keep_alive(None)
        .with_allowed_hosts(local(""))
        .with_allowed_origins(local("http://"))
        .enforce_origin_validation()
        .with_max_request_body_bytes(MAX_BODY);
    let monax = tools::Monax { reg, open };
    let mcp = StreamableHttpService::new(
        move || Ok(monax.clone()),
        Arc::new(NeverSessionManager::default()),
        cfg,
    );
    let token: Arc<str> = info.token.expose().into();
    let app = Router::new()
        .route_service("/mcp", mcp)
        .layer(middleware::from_fn_with_state(token, auth));
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Ok(Server { info, task })
}

const MAX_BODY: usize = 1 << 20;

/// 401 with an empty body unless `Authorization: Bearer <token>` matches.
async fn auth(State(token): State<Arc<str>>, req: Request, next: Next) -> Response {
    let ok = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|t| config::same(t.as_bytes(), token.as_bytes()));
    if ok {
        next.run(req).await
    } else {
        StatusCode::UNAUTHORIZED.into_response()
    }
}

/// Starts the app's server with `mcp.json` in `dir` and records the outcome in
/// [`McpState`] for Settings; the GUI works either way. `pdf_open_in_app`
/// emits `open-document` (the absolute path) and focuses the main window.
pub fn launch<R: Runtime>(app: &AppHandle<R>, dir: Option<PathBuf>) {
    let h = app.clone();
    let open: OpenHook = Arc::new(move |p| {
        let _ = h.emit("open-document", p);
        if let Some(w) = h.get_webview_window("main") {
            let _ = w.set_focus();
        }
    });
    let reg = app.state::<Registry>().inner().clone();
    let started = match dir {
        None => Err("no config directory: HOME unset or MONAX_CONFIG_DIR not absolute".into()),
        Some(dir) => tauri::async_runtime::block_on(start(&dir, reg, open))
            .map_err(|e| format!("{e} ({})", dir.display())),
    };
    app.state::<McpState>().set(match started {
        Ok(server) => McpStatus::Running(server.info), // the task keeps serving
        Err(reason) => McpStatus::Failed { reason },
    });
}

/// `$MONAX_CONFIG_DIR` (absolute; dev/test override), else
/// `~/Library/Application Support/monax`.
pub fn default_dir() -> Option<PathBuf> {
    config::dir(
        std::env::var_os("MONAX_CONFIG_DIR"),
        std::env::var_os("HOME"),
    )
}
