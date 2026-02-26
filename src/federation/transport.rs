use anyhow::Context;
use rmcp::service::RunningService;
use rmcp::RoleClient;
use rmcp::transport::StreamableHttpClientTransport;
use crate::config::DownstreamTransport;
use std::collections::HashMap;

/// Connect to a downstream MCP server via Streamable HTTP (localhost transport).
///
/// The downstream server must already be running and listening on the given URL.
pub async fn connect_localhost(
    url: &str,
) -> anyhow::Result<RunningService<RoleClient, ()>> {
    let transport = StreamableHttpClientTransport::from_uri(url);

    let client = rmcp::service::serve_client((), transport)
        .await
        .with_context(|| format!("Failed to initialize MCP client for: {}", url))?;

    Ok(client)
}

/// Spawn a downstream MCP server via stdio (child process) transport.
pub async fn connect_stdio(
    command: &str,
    args: &[String],
    env: &HashMap<String, String>,
) -> anyhow::Result<RunningService<RoleClient, ()>> {
    let mut cmd = tokio::process::Command::new(command);
    cmd.args(args).envs(env);

    let transport = rmcp::transport::TokioChildProcess::new(cmd)?;
    let client = rmcp::service::serve_client((), transport)
        .await
        .with_context(|| format!("MCP client init failed for stdio: {}", command))?;

    Ok(client)
}

/// Connect to a downstream based on its transport configuration.
pub async fn connect_downstream(
    transport: &DownstreamTransport,
) -> anyhow::Result<RunningService<RoleClient, ()>> {
    match transport {
        DownstreamTransport::Localhost { url } => connect_localhost(url).await,
        DownstreamTransport::Stdio { command, args, env } => {
            connect_stdio(command, args, env).await
        }
    }
}
