use tokio::sync::watch;

/// Spawn a background heartbeat task that sends periodic pings to cortexd.
///
/// Returns a shutdown sender — drop it or send () to stop the heartbeat loop.
pub fn spawn_heartbeat(
    cortexd_url: String,
    node_id: String,
    interval_secs: u64,
) -> watch::Sender<()> {
    let (tx, mut rx) = watch::channel(());

    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let url = format!("{}/api/v1/nodes/heartbeat", cortexd_url.trim_end_matches('/'));

        loop {
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(interval_secs)) => {
                    let payload = serde_json::json!({
                        "node_id": node_id,
                    });

                    match client
                        .post(&url)
                        .json(&payload)
                        .timeout(std::time::Duration::from_secs(5))
                        .send()
                        .await
                    {
                        Ok(resp) if resp.status().is_success() => {
                            tracing::debug!("Heartbeat sent successfully");
                        }
                        Ok(resp) => {
                            tracing::warn!(status = %resp.status(), "Heartbeat rejected by cortexd");
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "Heartbeat failed — cortexd unreachable");
                        }
                    }
                }
                _ = rx.changed() => {
                    tracing::info!("Heartbeat loop shutting down");
                    break;
                }
            }
        }
    });

    tx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_heartbeat_shutdown() {
        // Spawn heartbeat with a very long interval so it doesn't actually fire
        let tx = spawn_heartbeat(
            "http://localhost:9999".to_string(),
            "test-node".to_string(),
            3600, // 1 hour — won't fire during test
        );

        // Dropping the sender should cause the heartbeat task to stop
        drop(tx);

        // Give the task a moment to process the shutdown
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        // If we get here without hanging, the shutdown works
    }
}
