//! rmcp `ServerHandler`: lists [`TOOLS`] and dispatches calls off the async
//! workers. Tool results are structured JSON; tool failures are `isError`.
use super::tools::{Monax, TOOLS};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    InitializeResult, ListToolsResult, PaginatedRequestParams, ServerCapabilities, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler};
use serde_json::Value;

impl ServerHandler for Monax {
    fn get_info(&self) -> InitializeResult {
        InitializeResult::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("monax", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "Monax PDF editor. Paths are absolute. Outputs are new files (inputs are never \
                 modified). Coordinates are PDF user space: points, origin bottom-left, y-up, \
                 unrotated MediaBox.",
            )
    }

    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tools = TOOLS
            .iter()
            .map(|t| Tool::new(t.name, (t.about)(), (t.schema)()));
        Ok(ListToolsResult::with_all_items(tools.collect()))
    }

    async fn call_tool(
        &self,
        req: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let spec = TOOLS.iter().find(|t| t.name == req.name).ok_or_else(|| {
            ErrorData::invalid_params(format!("unknown tool: {}", req.name), None)
        })?;
        let args = Value::Object(req.arguments.unwrap_or_default());
        let this = self.clone();
        // Capabilities probes sidecars (up to 2s each): keep it off the async workers.
        let res = tokio::task::spawn_blocking(move || (spec.call)(&this, args))
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        Ok(match res {
            Ok(v) => CallToolResult::structured(v),
            Err(e) => CallToolResult::error(vec![ContentBlock::text(e)]),
        }
        .into())
    }
}
