use serde::Serialize;

/// Payload sent to cortexd on startup: POST /api/v1/nodes/register
#[derive(Debug, Serialize)]
pub struct RegisterPayload {
    pub node_id: String,
    pub hostname: String,
    pub ip_address: String,
    pub port: u16,
    pub capabilities: Vec<String>,
}

/// Register this neurond node with cortexd.
///
/// Sends a POST request with node identity, network address, and capabilities list.
/// Returns Ok(()) on success or an error if the orchestrator is unreachable.
pub async fn register_node(
    cortexd_url: &str,
    payload: &RegisterPayload,
) -> anyhow::Result<()> {
    let url = format!("{}/api/v1/nodes/register", cortexd_url.trim_end_matches('/'));

    tracing::info!(
        node_id = %payload.node_id,
        url = %url,
        "Registering with cortexd"
    );

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(payload)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?;

    if resp.status().is_success() {
        tracing::info!("Registered successfully with cortexd");
        Ok(())
    } else {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Registration failed: {} — {}", status, body)
    }
}

/// Deregister this node on graceful shutdown: POST /api/v1/nodes/deregister
pub async fn deregister_node(
    cortexd_url: &str,
    node_id: &str,
) -> anyhow::Result<()> {
    let url = format!("{}/api/v1/nodes/deregister", cortexd_url.trim_end_matches('/'));

    tracing::info!(node_id = %node_id, "Deregistering from cortexd");

    let client = reqwest::Client::new();
    let _resp = client
        .post(&url)
        .json(&serde_json::json!({ "node_id": node_id }))
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await;

    // Best-effort — don't fail shutdown if cortexd is unreachable
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_payload_serializes() {
        let payload = RegisterPayload {
            node_id: "test-id".to_string(),
            hostname: "testhost".to_string(),
            ip_address: "192.168.1.1".to_string(),
            port: 8443,
            capabilities: vec!["linux".to_string()],
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["node_id"], "test-id");
        assert_eq!(json["hostname"], "testhost");
        assert_eq!(json["port"], 8443);
        assert_eq!(json["capabilities"][0], "linux");
    }

    #[test]
    fn test_register_url_construction() {
        let base = "https://cortexd.example.com:8443/";
        let url = format!("{}/api/v1/nodes/register", base.trim_end_matches('/'));
        assert_eq!(url, "https://cortexd.example.com:8443/api/v1/nodes/register");
    }
}
