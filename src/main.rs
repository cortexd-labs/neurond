pub mod engine;
pub mod linux;
pub mod providers;
pub mod transport;

use crate::engine::registry::ProviderRegistry;
use crate::transport::mcp::McpTransport;
use crate::providers::{
    system::SystemProvider,
    service::ServiceProvider,
    process::ProcessProvider,
    log::LogProvider,
};

fn main() {
    // 1. Initialize the Provider Registry
    let mut registry = ProviderRegistry::new();

    // 2. Register all MVP Providers
    registry.register(Box::new(SystemProvider));
    registry.register(Box::new(ServiceProvider));
    registry.register(Box::new(ProcessProvider));
    registry.register(Box::new(LogProvider));

    // 3. Initialize the MCP stdio Transport layer
    let mcp = McpTransport::new(&registry);

    // 4. Run the JSON-RPC loop over stdin/stdout
    // The policy engine and audit logging are deferred as requested 
    // for MVP brevity, but the architecture supports inserting 
    // a Policy wrapper around the `registry.call_tool` invocation.
    if let Err(e) = mcp.run_stdio_loop() {
        eprintln!("cortexd stdio loop exited with error: {}", e);
    }
}
