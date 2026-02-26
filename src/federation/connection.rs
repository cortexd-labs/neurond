use rmcp::model::Tool;
use rmcp::service::RunningService;
use rmcp::RoleClient;
use std::time::Instant;

/// Lifecycle state of a downstream MCP server connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Configuration loaded, not yet connected
    Configured,
    /// Spawn/connect in progress
    Starting,
    /// Connected and tools discovered
    Healthy,
    /// Connection lost, attempting reconnect
    Restarting { attempt: u32 },
    /// Max retries exceeded — tools removed from registry
    Failed,
}

/// Represents a live connection to a downstream MCP server.
pub struct DownstreamConnection {
    /// Namespace prefix for this downstream's tools
    pub namespace: String,
    /// Current lifecycle state
    pub state: ConnectionState,
    /// Cached list of tools from this downstream (namespaced)
    pub tools: Vec<Tool>,
    /// Active rmcp client service handle
    pub client: Option<RunningService<RoleClient, ()>>,
    /// Spawned child process handle (for stdio transport)
    pub child: Option<tokio::process::Child>,
    /// Last successful health check timestamp
    pub last_seen: Instant,
}

impl DownstreamConnection {
    pub fn new(namespace: String) -> Self {
        Self {
            namespace,
            state: ConnectionState::Configured,
            tools: Vec::new(),
            client: None,
            child: None,
            last_seen: Instant::now(),
        }
    }

    /// Returns true if the connection is healthy and ready to serve requests.
    pub fn is_healthy(&self) -> bool {
        self.state == ConnectionState::Healthy && self.client.is_some()
    }

    /// Transition to the Starting state.
    pub fn mark_starting(&mut self) {
        self.state = ConnectionState::Starting;
    }

    /// Transition to the Healthy state and cache the tool list.
    pub fn mark_healthy(&mut self, tools: Vec<Tool>) {
        self.state = ConnectionState::Healthy;
        self.tools = tools;
        self.last_seen = Instant::now();
    }

    /// Transition to the Restarting state, incrementing retry count.
    pub fn mark_restarting(&mut self) {
        let attempt = match &self.state {
            ConnectionState::Restarting { attempt } => attempt + 1,
            _ => 1,
        };
        self.state = ConnectionState::Restarting { attempt };
        self.tools.clear();
    }

    /// Transition to the Failed state — gives up reconnecting.
    pub fn mark_failed(&mut self) {
        self.state = ConnectionState::Failed;
        self.tools.clear();
        self.client = None;
    }
}
