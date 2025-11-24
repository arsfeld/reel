use crate::db::DatabaseConnection;
use crate::models::{Credentials, ServerConnection, ServerConnections, SourceId};
use crate::services::core::auth::AuthService;
use crate::services::core::connection_cache::{ConnectionCache, ConnectionState, ConnectionType};
use anyhow::Result;
use std::sync::Arc;
use tracing::{debug, info, warn};

// Global connection cache - shared across all backends
lazy_static::lazy_static! {
    static ref CONNECTION_CACHE: Arc<ConnectionCache> = Arc::new(ConnectionCache::new());
}

/// Service for managing and selecting optimal server connections
pub struct ConnectionService;

impl ConnectionService {
    /// Get the global connection cache
    pub fn cache() -> Arc<ConnectionCache> {
        CONNECTION_CACHE.clone()
    }

    /// Test all connections for a source and update the best one
    pub async fn select_best_connection(
        db: &DatabaseConnection,
        source_id: &SourceId,
    ) -> Result<Option<String>> {
        // Check cache first
        let cache = Self::cache();
        if cache.should_skip_test(source_id).await
            && let Some(state) = cache.get(source_id).await
        {
            debug!(
                "Using cached connection for {}: {} (age: {:?})",
                source_id,
                state.url,
                state.age()
            );
            return Ok(Some(state.url));
        }
        use crate::db::repository::Repository;
        use crate::db::repository::source_repository::{SourceRepository, SourceRepositoryImpl};

        let repo = SourceRepositoryImpl::new(db.clone());
        let source = Repository::find_by_id(&repo, source_id.as_ref())
            .await?
            .ok_or_else(|| anyhow::anyhow!("Source not found"))?;

        // Get auth token for the source (needed for authenticated connection testing)
        let auth_token = {
            use crate::db::repository::auth_token_repository::{
                AuthTokenRepository, AuthTokenRepositoryImpl,
            };

            let repo = AuthTokenRepositoryImpl::new(db.clone());

            // Legacy support: some tokens may still be stored under "auth"
            if let Some(token) = repo
                .find_by_source_and_type(source_id.as_ref(), "auth")
                .await?
            {
                Some(token.token)
            } else if let Some(token) = repo
                .find_by_source_and_type(source_id.as_ref(), "token")
                .await?
            {
                Some(token.token)
            } else if let Some(token) = repo
                .find_by_source_and_type(source_id.as_ref(), "access")
                .await?
            {
                Some(token.token)
            } else {
                match AuthService::load_credentials(db, source_id).await? {
                    Some(Credentials::Token { token }) => Some(token),
                    Some(Credentials::ApiKey { key }) => Some(key),
                    _ => None,
                }
            }
        };

        // Get stored connections from JSON column
        if let Some(ref connections_json) = source.connections {
            let connections: ServerConnections = serde_json::from_value(connections_json.clone())?;

            // Test all connections in parallel
            let tested_connections =
                Self::test_connections(db, &source, connections.connections, auth_token.as_deref())
                    .await;

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
                repo.update_connection_url(source_id.as_ref(), Some(best_conn.uri.clone()))
                    .await?;

                // Store updated connections with test results
                repo.update_connections(
                    source_id.as_ref(),
                    serde_json::to_value(&server_connections)?,
                )
                .await?;

                // Update cache with successful connection
                let conn_type = ConnectionType::from_connection(best_conn);
                let state = ConnectionState::new(
                    best_conn.uri.clone(),
                    conn_type,
                    best_conn.response_time_ms.unwrap_or(0),
                );
                cache.insert(source_id.clone(), state).await;

                // Update database connection tracking fields
                use crate::db::entities::sources::ActiveModel;
                use sea_orm::{ActiveModelTrait, Set};

                let active_model = ActiveModel {
                    id: Set(source_id.to_string()),
                    last_connection_test: Set(Some(chrono::Utc::now().naive_utc())),
                    connection_failure_count: Set(0),
                    connection_quality: Set(Some(conn_type.to_string())),
                    ..Default::default()
                };
                active_model.update(db.as_ref()).await?;

                return Ok(Some(best_conn.uri.clone()));
            }
        }

        // Fall back to the existing connection_url if no connections stored
        Ok(source.connection_url)
    }

    /// Test multiple connections in parallel using backend-specific logic
    async fn test_connections(
        db: &DatabaseConnection,
        source: &crate::db::entities::sources::Model,
        connections: Vec<ServerConnection>,
        auth_token: Option<&str>,
    ) -> Vec<ServerConnection> {
        use crate::services::core::backend::BackendService;

        // Create backend for this source
        let _backend = match BackendService::create_backend_for_source(db, source).await {
            Ok(backend) => backend,
            Err(e) => {
                warn!("Failed to create backend for connection testing: {}", e);
                return connections; // Return unchanged connections if backend creation fails
            }
        };

        let mut handles = Vec::new();

        for mut conn in connections {
            let auth_token = auth_token.map(|t| t.to_string());
            let uri = conn.uri.clone();

            // Clone necessary data for the async task
            // We need to create a new backend instance for each task since MediaBackend isn't Clone
            let db = db.clone();
            let source = source.clone();

            let handle = tokio::spawn(async move {
                let conn_type = if conn.local {
                    "local"
                } else if conn.relay {
                    "relay"
                } else {
                    "remote"
                };

                debug!(
                    "Testing {} connection: {} (auth: {})",
                    conn_type,
                    uri,
                    if auth_token.is_some() { "YES" } else { "NO" }
                );

                // Create a backend instance for this task
                let backend = match BackendService::create_backend_for_source(&db, &source).await {
                    Ok(backend) => backend,
                    Err(e) => {
                        warn!("Failed to create backend for connection test: {}", e);
                        conn.is_available = false;
                        conn.response_time_ms = None;
                        return conn;
                    }
                };

                // Use backend's test_connection method
                match backend.test_connection(&uri, auth_token.as_deref()).await {
                    Ok((is_available, response_time_ms)) => {
                        conn.is_available = is_available;
                        conn.response_time_ms = response_time_ms;
                        if is_available {
                            info!(
                                "✓ {} connection {} available ({}ms)",
                                conn_type,
                                uri,
                                response_time_ms.unwrap_or(0)
                            );
                        } else {
                            debug!("✗ {} connection {} not available", conn_type, uri);
                        }
                    }
                    Err(e) => {
                        conn.is_available = false;
                        conn.response_time_ms = None;
                        debug!("✗ {} connection {} failed: {}", conn_type, uri, e);
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
