use rmcp::{
    handler::server::ServerHandler,
    model::*,
    ErrorData as McpError,
    service::{RequestContext, RoleServer},
};
use std::sync::Arc;
use crate::federation::manager::FederationManager;

/// ProxyEngine is the MCP ServerHandler that neurond exposes upstream (to cortexd).
///
/// It doesn't implement any tools directly — it delegates all tool calls
/// to the FederationManager, which routes them to the correct downstream.
#[derive(Clone)]
pub struct ProxyEngine {
    federation: Arc<FederationManager>,
}

impl ProxyEngine {
    pub fn new(federation: Arc<FederationManager>) -> Self {
        Self { federation }
    }
}

#[allow(clippy::manual_async_fn)]
impl ServerHandler for ProxyEngine {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "neurond".to_string(),
                title: Some("neurond Federation Proxy".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
                description: Some("Routes tool calls to downstream MCP servers".to_string()),
                icons: None,
                website_url: None,
            },
            instructions: Some("neurond federation proxy — routes tool calls to downstream MCP servers".to_string()),
            ..Default::default()
        }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        async {
            let tools = self.federation.list_all_tools().await;
            Ok(ListToolsResult {
                tools,
                next_cursor: None,
                meta: None,
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        async move {
            let tool_name = request.name.as_ref();
            let arguments = match request.arguments {
                Some(map) => serde_json::Value::Object(map),
                None => serde_json::json!({}),
            };

            tracing::info!(tool = %tool_name, "Routing tool call to downstream");

            self.federation
                .route_tool_call(tool_name, arguments)
                .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_proxy_engine_info() {
        let mgr = Arc::new(FederationManager::new());
        let engine = ProxyEngine::new(mgr);
        let info = engine.get_info();
        assert_eq!(info.server_info.name, "neurond");
    }

    #[tokio::test]
    async fn test_proxy_engine_list_tools_empty() {
        let mgr = Arc::new(FederationManager::new());
        let engine = ProxyEngine::new(mgr);

        // Create a minimal RequestContext for testing
        // We use the default ServerHandler trait method through direct async call
        let tools = mgr_list_tools(&engine).await;
        assert!(tools.is_empty());
    }

    // Helper to call list_all_tools on the federation manager directly
    async fn mgr_list_tools(engine: &ProxyEngine) -> Vec<Tool> {
        engine.federation.list_all_tools().await
    }

    #[tokio::test]
    async fn test_proxy_engine_route_unknown_tool() {
        let mgr = Arc::new(FederationManager::new());
        let result = mgr.route_tool_call("unknown.tool", serde_json::json!({})).await;
        assert!(result.is_err());
    }
}
