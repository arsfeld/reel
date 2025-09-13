use anyhow::Result;
use relm4::{ComponentSender, Worker};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info};

use crate::backends::traits::MediaBackend;
use crate::db::connection::DatabaseConnection;
use crate::models::Credentials;
use crate::models::SourceId;
use crate::services::core::auth::AuthService;

/// Messages that can be sent to the ConnectionWorker
#[derive(Debug, Clone)]
pub enum ConnectionWorkerInput {
    /// Test connection to a backend
    TestConnection {
        source_id: SourceId,
        backend: Arc<dyn MediaBackend>,
        credentials: Credentials,
    },
    /// Monitor connection status
    StartMonitoring {
        source_id: SourceId,
        backend: Arc<dyn MediaBackend>,
        interval: Duration,
    },
    /// Stop monitoring a connection
    StopMonitoring(SourceId),
    /// Re-authenticate a source
    Reauth {
        source_id: SourceId,
        backend: Arc<dyn MediaBackend>,
    },
}

/// Messages sent from the ConnectionWorker
#[derive(Debug, Clone)]
pub enum ConnectionWorkerOutput {
    /// Connection test result
    ConnectionStatus {
        source_id: SourceId,
        is_connected: bool,
        error: Option<String>,
    },
    /// Connection state changed
    ConnectionChanged {
        source_id: SourceId,
        is_connected: bool,
    },
    /// Re-authentication result
    ReauthResult {
        source_id: SourceId,
        success: bool,
        error: Option<String>,
    },
}

/// Worker for managing backend connections
pub struct ConnectionWorker {
    db: DatabaseConnection,
    monitoring: HashMap<SourceId, bool>,
}

impl ConnectionWorker {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            monitoring: HashMap::new(),
        }
    }

    async fn test_connection(
        &self,
        source_id: SourceId,
        backend: Arc<dyn MediaBackend>,
        credentials: Credentials,
        sender: ComponentSender<Self>,
    ) {
        debug!("Testing connection for source: {}", source_id);

        match AuthService::test_connection(backend.as_ref(), credentials).await {
            Ok(is_connected) => {
                let _ = sender.output(ConnectionWorkerOutput::ConnectionStatus {
                    source_id,
                    is_connected,
                    error: None,
                });
            }
            Err(e) => {
                error!("Connection test failed for {}: {}", source_id, e);
                let _ = sender.output(ConnectionWorkerOutput::ConnectionStatus {
                    source_id,
                    is_connected: false,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    async fn monitor_connection(
        &mut self,
        source_id: SourceId,
        backend: Arc<dyn MediaBackend>,
        interval: Duration,
        sender: ComponentSender<Self>,
    ) {
        info!("Starting connection monitoring for source: {}", source_id);
        self.monitoring.insert(source_id.clone(), true);

        let mut last_status = false;

        while self.monitoring.get(&source_id) == Some(&true) {
            // Load credentials
            let credentials = match AuthService::load_credentials(&source_id).await {
                Ok(Some(creds)) => creds,
                Ok(None) => {
                    error!("No credentials found for source: {}", source_id);
                    break;
                }
                Err(e) => {
                    error!("Failed to load credentials: {}", e);
                    break;
                }
            };

            // Test connection
            let is_connected = AuthService::test_connection(backend.as_ref(), credentials)
                .await
                .unwrap_or(false);

            // Notify if status changed
            if is_connected != last_status {
                let _ = sender.output(ConnectionWorkerOutput::ConnectionChanged {
                    source_id: source_id.clone(),
                    is_connected,
                });
                last_status = is_connected;
            }

            // Wait for next check
            tokio::time::sleep(interval).await;
        }

        info!("Stopped monitoring connection for source: {}", source_id);
    }

    async fn reauth_source(
        &self,
        source_id: SourceId,
        backend: Arc<dyn MediaBackend>,
        sender: ComponentSender<Self>,
    ) {
        info!("Re-authenticating source: {}", source_id);

        match AuthService::reauth_source(&self.db, backend.as_ref(), &source_id).await {
            Ok(_) => {
                let _ = sender.output(ConnectionWorkerOutput::ReauthResult {
                    source_id,
                    success: true,
                    error: None,
                });
            }
            Err(e) => {
                error!("Re-authentication failed for {}: {}", source_id, e);
                let _ = sender.output(ConnectionWorkerOutput::ReauthResult {
                    source_id,
                    success: false,
                    error: Some(e.to_string()),
                });
            }
        }
    }
}

impl Worker for ConnectionWorker {
    type Init = DatabaseConnection;
    type Input = ConnectionWorkerInput;
    type Output = ConnectionWorkerOutput;

    fn init(db: Self::Init, _sender: ComponentSender<Self>) -> Self {
        Self::new(db)
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            ConnectionWorkerInput::TestConnection {
                source_id,
                backend,
                credentials,
            } => {
                let worker = self.clone();
                relm4::spawn(async move {
                    worker
                        .test_connection(source_id, backend, credentials, sender)
                        .await;
                });
            }
            ConnectionWorkerInput::StartMonitoring {
                source_id,
                backend,
                interval,
            } => {
                let mut worker = self.clone();
                relm4::spawn(async move {
                    worker
                        .monitor_connection(source_id, backend, interval, sender)
                        .await;
                });
            }
            ConnectionWorkerInput::StopMonitoring(source_id) => {
                self.monitoring.insert(source_id.clone(), false);
                info!("Requested to stop monitoring: {}", source_id);
            }
            ConnectionWorkerInput::Reauth { source_id, backend } => {
                let worker = self.clone();
                relm4::spawn(async move {
                    worker.reauth_source(source_id, backend, sender).await;
                });
            }
        }
    }
}

impl Clone for ConnectionWorker {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            monitoring: self.monitoring.clone(),
        }
    }
}
