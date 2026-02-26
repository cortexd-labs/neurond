use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::{DownstreamServer, FederationConfig};
use crate::federation::connection::{ConnectionState, DownstreamConnection};
use crate::federation::namespace;
use crate::federation::transport;
use rmcp::model::{CallToolRequestParams, CallToolResult, Tool};

/// Maximum reconnection attempts before marking a downstream as Failed.
const MAX_RETRIES: u32 = 5;

/// Manages all downstream MCP server connections.
///
/// The FederationManager is responsible for:
/// 1. Spawning/connecting to downstream MCP servers
/// 2. Discovering their tool registries
/// 3. Aggregating tools under namespaces
/// 4. Routing tool calls to the correct downstream
pub struct FederationManager {
    downstreams: Arc<RwLock<Vec<DownstreamConnection>>>,
}

impl Default for FederationManager {
    fn default() -> Self {
        Self {
            downstreams: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl FederationManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize all downstream connections from config.
    pub async fn init_from_config(&self, config: &FederationConfig) -> anyhow::Result<()> {
        for server_config in &config.servers {
            self.add_downstream(server_config).await;
        }
        Ok(())
    }

    /// Add and connect a single downstream MCP server.
    async fn add_downstream(&self, config: &DownstreamServer) {
        let namespace = config.namespace.clone();
        tracing::info!(namespace = %namespace, "Connecting to downstream MCP server");

        let mut conn = DownstreamConnection::new(namespace.clone());
        conn.mark_starting();

        match transport::connect_downstream(&config.transport).await {
            Ok(client) => {
                // Discover tools from the downstream via the peer handle
                match client.peer().list_all_tools().await {
                    Ok(raw_tools) => {
                        let namespaced = namespace::namespace_tools(&namespace, &raw_tools);
                        let count = namespaced.len();
                        conn.mark_healthy(namespaced);
                        conn.client = Some(client);
                        tracing::info!(
                            namespace = %namespace,
                            tools = count,
                            "Downstream connected — {} tools registered",
                            count
                        );
                    }
                    Err(e) => {
                        tracing::error!(namespace = %namespace, error = %e, "Failed to list tools from downstream");
                        conn.mark_failed();
                    }
                }
            }
            Err(e) => {
                tracing::error!(namespace = %namespace, error = %e, "Failed to connect to downstream");
                conn.mark_failed();
            }
        }

        self.downstreams.write().await.push(conn);
    }

    /// Get the aggregated tool list from all healthy downstreams.
    pub async fn list_all_tools(&self) -> Vec<Tool> {
        let downstreams = self.downstreams.read().await;
        downstreams
            .iter()
            .filter(|c| c.is_healthy())
            .flat_map(|c| c.tools.clone())
            .collect()
    }

    /// Get the list of all known namespace strings.
    pub async fn namespaces(&self) -> Vec<String> {
        let downstreams = self.downstreams.read().await;
        downstreams.iter().map(|c| c.namespace.clone()).collect()
    }

    /// Route a tool call to the correct downstream by namespace.
    ///
    /// Strips the namespace prefix, forwards the call, and returns the result.
    pub async fn route_tool_call(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let downstreams = self.downstreams.read().await;
        let namespaces: Vec<String> = downstreams.iter().map(|c| c.namespace.clone()).collect();

        let (target_ns, original_name) =
            namespace::resolve_namespace(&namespaces, tool_name).ok_or_else(|| {
                rmcp::ErrorData {
                    code: rmcp::model::ErrorCode::METHOD_NOT_FOUND,
                    message: format!("No downstream registered for tool: {tool_name}").into(),
                    data: None,
                }
            })?;

        let conn = downstreams
            .iter()
            .find(|c| c.namespace == target_ns && c.is_healthy())
            .ok_or_else(|| rmcp::ErrorData {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("Downstream '{target_ns}' is not healthy").into(),
                data: None,
            })?;

        let client = conn.client.as_ref().ok_or_else(|| rmcp::ErrorData {
            code: rmcp::model::ErrorCode::INTERNAL_ERROR,
            message: format!("No active client for downstream '{target_ns}'").into(),
            data: None,
        })?;

        // Build the downstream call params
        let params = CallToolRequestParams {
            name: original_name.into(),
            arguments: arguments.as_object().cloned(),
            meta: None,
            task: None,
        };

        let result = client
            .peer()
            .call_tool(params)
            .await
            .map_err(|e| rmcp::ErrorData {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: format!("Downstream '{target_ns}' error: {e}").into(),
                data: None,
            })?;

        Ok(result)
    }

    /// Get status of all downstream connections (for diagnostics).
    pub async fn status_summary(&self) -> Vec<(String, String)> {
        let downstreams = self.downstreams.read().await;
        downstreams
            .iter()
            .map(|c| {
                let state = match &c.state {
                    ConnectionState::Configured => "configured",
                    ConnectionState::Starting => "starting",
                    ConnectionState::Healthy => "healthy",
                    ConnectionState::Restarting { .. } => "restarting",
                    ConnectionState::Failed => "failed",
                };
                (c.namespace.clone(), state.to_string())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_manager_new_has_no_downstreams() {
        let mgr = FederationManager::new();
        assert!(mgr.list_all_tools().await.is_empty());
        assert!(mgr.namespaces().await.is_empty());
    }

    #[tokio::test]
    async fn test_manager_route_unknown_namespace_returns_error() {
        let mgr = FederationManager::new();
        let result = mgr
            .route_tool_call("unknown.tool", serde_json::json!({}))
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, rmcp::model::ErrorCode::METHOD_NOT_FOUND);
    }

    #[tokio::test]
    async fn test_manager_status_summary_empty() {
        let mgr = FederationManager::new();
        let summary = mgr.status_summary().await;
        assert!(summary.is_empty());
    }

    #[tokio::test]
    async fn test_manager_init_empty_config() {
        let mgr = FederationManager::new();
        let config = FederationConfig::default();
        mgr.init_from_config(&config).await.unwrap();
        assert!(mgr.namespaces().await.is_empty());
    }
}
