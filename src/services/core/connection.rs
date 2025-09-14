use crate::db::DatabaseConnection;
use crate::models::{ServerConnection, ServerConnections, Source, SourceId};
use anyhow::Result;
use reqwest::Client;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Service for managing and selecting optimal server connections
pub struct ConnectionService;

impl ConnectionService {
    /// Test all connections for a source and update the best one
    pub async fn select_best_connection(
        db: &DatabaseConnection,
        source_id: &SourceId,
    ) -> Result<Option<String>> {
        use crate::db::repository::Repository;
        use crate::db::repository::source_repository::{SourceRepository, SourceRepositoryImpl};

        let repo = SourceRepositoryImpl::new(db.clone());
        let source = Repository::find_by_id(&repo, &source_id.to_string())
            .await?
            .ok_or_else(|| anyhow::anyhow!("Source not found"))?;

        // Get stored connections from JSON column
        if let Some(connections_json) = source.connections {
            let connections: ServerConnections = serde_json::from_value(connections_json)?;

            // Test all connections in parallel
            let tested_connections = Self::test_connections(connections.connections).await;

            // Find the best available connection
            let server_connections = ServerConnections::new(tested_connections);
            let best = server_connections.best_connection();

            if let Some(best_conn) = best {
                info!(
                    "Selected best connection for {}: {} (local: {}, relay: {}, response: {:?}ms)",
                    source_id,
                    best_conn.uri,
                    best_conn.local,
                    best_conn.relay,
                    best_conn.response_time_ms
                );

                // Update the primary URL in the database
                repo.update_connection_url(&source_id.to_string(), Some(best_conn.uri.clone()))
                    .await?;

                // Store updated connections with test results
                repo.update_connections(
                    &source_id.to_string(),
                    serde_json::to_value(&server_connections)?,
                )
                .await?;

                return Ok(Some(best_conn.uri.clone()));
            }
        }

        // Fall back to the existing connection_url if no connections stored
        Ok(source.connection_url)
    }

    /// Test multiple connections in parallel
    async fn test_connections(connections: Vec<ServerConnection>) -> Vec<ServerConnection> {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        let mut handles = Vec::new();

        for mut conn in connections {
            let client = client.clone();
            let handle = tokio::spawn(async move {
                let start = Instant::now();

                // Test the connection
                let test_url = if conn.uri.contains("plex.tv") {
                    // For Plex, test the identity endpoint
                    format!("{}/identity", conn.uri.trim_end_matches('/'))
                } else {
                    // For other servers, just test the root
                    conn.uri.clone()
                };

                match client.get(&test_url).send().await {
                    Ok(response) if response.status().is_success() => {
                        conn.is_available = true;
                        conn.response_time_ms = Some(start.elapsed().as_millis() as u64);
                        debug!(
                            "Connection {} available ({}ms)",
                            conn.uri,
                            conn.response_time_ms.unwrap_or(0)
                        );
                    }
                    Ok(response) => {
                        conn.is_available = false;
                        conn.response_time_ms = None;
                        warn!(
                            "Connection {} returned status: {}",
                            conn.uri,
                            response.status()
                        );
                    }
                    Err(e) => {
                        conn.is_available = false;
                        conn.response_time_ms = None;
                        debug!("Connection {} failed: {}", conn.uri, e);
                    }
                }

                conn
            });

            handles.push(handle);
        }

        // Wait for all tests to complete
        let mut results = Vec::new();
        for handle in handles {
            if let Ok(conn) = handle.await {
                results.push(conn);
            }
        }

        results
    }

    /// Convert Plex connections to our ServerConnection model
    pub fn from_plex_connections(
        plex_connections: Vec<crate::backends::plex::PlexConnection>,
    ) -> Vec<ServerConnection> {
        plex_connections
            .into_iter()
            .enumerate()
            .map(|(idx, conn)| {
                let mut priority = idx as i32 * 10;

                // Adjust priority based on connection type
                if conn.local && !conn.relay {
                    priority -= 1000; // Prefer local non-relay
                } else if !conn.relay {
                    priority -= 500; // Prefer direct connections
                }

                ServerConnection {
                    uri: conn.uri,
                    protocol: conn.protocol,
                    address: conn.address,
                    port: conn.port as u32,
                    local: conn.local,
                    relay: conn.relay,
                    priority,
                    is_available: false, // Will be tested
                    response_time_ms: None,
                }
            })
            .collect()
    }
}
