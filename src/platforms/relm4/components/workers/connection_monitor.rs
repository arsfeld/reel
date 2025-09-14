use relm4::Worker;
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::db::DatabaseConnection;
use crate::models::SourceId;
use crate::services::core::ConnectionService;

pub struct ConnectionMonitor {
    db: DatabaseConnection,
    interval: Duration,
}

#[derive(Debug, Clone)]
pub enum ConnectionMonitorInput {
    CheckSource(SourceId),
    CheckAllSources,
    Stop,
}

#[derive(Debug, Clone)]
pub enum ConnectionMonitorOutput {
    ConnectionChanged {
        source_id: SourceId,
        new_url: String,
    },
    ConnectionLost {
        source_id: SourceId,
    },
    ConnectionRestored {
        source_id: SourceId,
        url: String,
    },
}

impl Worker for ConnectionMonitor {
    type Init = DatabaseConnection;
    type Input = ConnectionMonitorInput;
    type Output = ConnectionMonitorOutput;

    fn init(db: Self::Init, _sender: relm4::ComponentSender<Self>) -> Self {
        Self {
            db,
            interval: Duration::from_secs(30), // Check every 30 seconds
        }
    }

    fn update(&mut self, msg: Self::Input, sender: relm4::ComponentSender<Self>) {
        match msg {
            ConnectionMonitorInput::CheckSource(source_id) => {
                let db = self.db.clone();
                let sender = sender.clone();

                tokio::spawn(async move {
                    info!("Checking connections for source: {}", source_id);

                    match ConnectionService::select_best_connection(&db, &source_id).await {
                        Ok(Some(new_url)) => {
                            info!("Selected connection for {}: {}", source_id, new_url);
                            let _ = sender.output(ConnectionMonitorOutput::ConnectionRestored {
                                source_id: source_id.clone(),
                                url: new_url.clone(),
                            });
                        }
                        Ok(None) => {
                            warn!("No available connections for source: {}", source_id);
                            let _ = sender.output(ConnectionMonitorOutput::ConnectionLost {
                                source_id: source_id.clone(),
                            });
                        }
                        Err(e) => {
                            warn!("Failed to check connections for {}: {}", source_id, e);
                        }
                    }
                });
            }

            ConnectionMonitorInput::CheckAllSources => {
                use crate::db::repository::Repository;
                use crate::db::repository::source_repository::SourceRepositoryImpl;

                let db = self.db.clone();
                let sender = sender.clone();

                tokio::spawn(async move {
                    let repo = SourceRepositoryImpl::new(db.clone());

                    match Repository::find_all(&repo).await {
                        Ok(sources) => {
                            info!("Checking connections for {} sources", sources.len());

                            for source in sources {
                                let source_id = SourceId::new(source.id);

                                // Store the previous URL
                                let previous_url = source.connection_url.clone();

                                // Check and select best connection
                                match ConnectionService::select_best_connection(&db, &source_id)
                                    .await
                                {
                                    Ok(Some(new_url)) => {
                                        // Check if URL changed
                                        if previous_url.as_ref() != Some(&new_url) {
                                            info!(
                                                "Connection changed for {}: {:?} -> {}",
                                                source_id, previous_url, new_url
                                            );
                                            let _ = sender.output(
                                                ConnectionMonitorOutput::ConnectionChanged {
                                                    source_id: source_id.clone(),
                                                    new_url: new_url.clone(),
                                                },
                                            );
                                        } else {
                                            debug!(
                                                "Connection unchanged for {}: {}",
                                                source_id, new_url
                                            );
                                        }
                                    }
                                    Ok(None) => {
                                        if previous_url.is_some() {
                                            warn!("Lost all connections for source: {}", source_id);
                                            let _ = sender.output(
                                                ConnectionMonitorOutput::ConnectionLost {
                                                    source_id: source_id.clone(),
                                                },
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        warn!(
                                            "Failed to check connections for {}: {}",
                                            source_id, e
                                        );
                                    }
                                }

                                // Small delay between sources to avoid overwhelming
                                tokio::time::sleep(Duration::from_millis(100)).await;
                            }
                        }
                        Err(e) => {
                            warn!("Failed to get sources for connection monitoring: {}", e);
                        }
                    }
                });
            }

            ConnectionMonitorInput::Stop => {
                info!("Stopping connection monitor");
            }
        }
    }
}

impl ConnectionMonitor {
    /// Start periodic monitoring of all sources
    pub fn start_monitoring(sender: relm4::ComponentSender<ConnectionMonitor>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                interval.tick().await;

                // Send check all sources message
                sender.input(ConnectionMonitorInput::CheckAllSources);
            }
        });
    }
}
