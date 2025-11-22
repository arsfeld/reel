use relm4::Worker;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::runtime::Handle;
use tracing::{debug, info, warn};

use crate::db::DatabaseConnection;
use crate::models::{AuthStatus, SourceId};
use crate::services::core::connection::ConnectionService;
use crate::services::core::connection_cache::ConnectionType;

#[derive(Debug)]
pub struct ConnectionMonitor {
    pub(crate) db: DatabaseConnection,
    pub(crate) runtime: Handle,
    pub(crate) next_check_times: HashMap<SourceId, Instant>,
    pub(crate) last_auth_status: HashMap<SourceId, AuthStatus>,
}

#[derive(Debug, Clone)]
pub enum ConnectionMonitorInput {
    CheckSource(SourceId),
    CheckAllSources,
    UpdateCheckTimes(HashMap<SourceId, Instant>),
    UpdateAuthStatus(HashMap<SourceId, AuthStatus>),
}

#[derive(Debug, Clone)]
pub enum ConnectionMonitorOutput {
    ConnectionChanged {
        source_id: SourceId,
        new_url: String,
        connection_type: ConnectionType,
    },
    ConnectionLost {
        source_id: SourceId,
    },
    ConnectionRestored {
        source_id: SourceId,
        url: String,
        connection_type: ConnectionType,
    },
    AuthStatusChanged {
        source_id: SourceId,
        needs_auth: bool,
    },
}

impl Worker for ConnectionMonitor {
    type Init = (DatabaseConnection, Handle);
    type Input = ConnectionMonitorInput;
    type Output = ConnectionMonitorOutput;

    fn init((db, runtime): Self::Init, _sender: relm4::ComponentSender<Self>) -> Self {
        Self {
            db,
            runtime,
            next_check_times: HashMap::new(),
            last_auth_status: HashMap::new(),
        }
    }

    fn update(&mut self, msg: Self::Input, sender: relm4::ComponentSender<Self>) {
        match msg {
            ConnectionMonitorInput::CheckSource(source_id) => {
                let db = self.db.clone();
                let sender = sender.clone();
                let runtime = self.runtime.clone();

                runtime.spawn(async move {
                    info!("Checking connections for source: {}", source_id);

                    match ConnectionService::select_best_connection(&db, &source_id).await {
                        Ok(Some(new_url)) => {
                            info!("Selected connection for {}: {}", source_id, new_url);

                            // Get connection type from cache
                            let cache = ConnectionService::cache();
                            let connection_type = if let Some(state) = cache.get(&source_id).await {
                                state.connection_type
                            } else {
                                ConnectionType::Remote // Default to remote if not in cache
                            };

                            let _ = sender.output(ConnectionMonitorOutput::ConnectionRestored {
                                source_id: source_id.clone(),
                                url: new_url.clone(),
                                connection_type,
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
                let runtime = self.runtime.clone();
                let mut next_check_times = self.next_check_times.clone();
                let mut last_auth_status = self.last_auth_status.clone();

                runtime.spawn(async move {
                    let repo = SourceRepositoryImpl::new(db.clone());

                    match Repository::find_all(&repo).await {
                        Ok(sources) => {
                            let mut sources_to_check = 0;

                            for source in sources {
                                let source_id = SourceId::new(source.id.clone());

                                // Check for auth status changes
                                let current_auth_status =
                                    AuthStatus::from(source.auth_status.clone());
                                let prev_auth_status = last_auth_status.get(&source_id).copied();

                                if prev_auth_status.is_none()
                                    || prev_auth_status != Some(current_auth_status)
                                {
                                    debug!(
                                        "Auth status changed for {}: {:?} -> {:?}",
                                        source_id, prev_auth_status, current_auth_status
                                    );

                                    // Emit auth status change if moving to AuthRequired
                                    if current_auth_status == AuthStatus::AuthRequired {
                                        info!(
                                            "Source {} now requires re-authentication",
                                            source_id
                                        );
                                        let _ = sender.output(
                                            ConnectionMonitorOutput::AuthStatusChanged {
                                                source_id: source_id.clone(),
                                                needs_auth: true,
                                            },
                                        );
                                    } else if prev_auth_status == Some(AuthStatus::AuthRequired)
                                        && current_auth_status == AuthStatus::Authenticated
                                    {
                                        info!("Source {} authentication restored", source_id);
                                        let _ = sender.output(
                                            ConnectionMonitorOutput::AuthStatusChanged {
                                                source_id: source_id.clone(),
                                                needs_auth: false,
                                            },
                                        );
                                    }

                                    last_auth_status.insert(source_id.clone(), current_auth_status);
                                }

                                // Check if this source is due for checking
                                let should_check = next_check_times
                                    .get(&source_id)
                                    .map(|&next_check| Instant::now() >= next_check)
                                    .unwrap_or(true);

                                if !should_check {
                                    continue;
                                }

                                sources_to_check += 1;

                                // Store the previous URL and quality
                                let previous_url = source.connection_url.clone();
                                let previous_quality = source.connection_quality.clone();

                                // Check and select best connection
                                match ConnectionService::select_best_connection(&db, &source_id)
                                    .await
                                {
                                    Ok(Some(new_url)) => {
                                        // Get connection type from cache
                                        let cache = ConnectionService::cache();
                                        let connection_type =
                                            if let Some(state) = cache.get(&source_id).await {
                                                state.connection_type
                                            } else {
                                                ConnectionType::Remote // Default to remote if not in cache
                                            };

                                        // Check if URL changed or quality changed
                                        let quality_changed = previous_quality.as_deref()
                                            != Some(&connection_type.to_string());

                                        if previous_url.as_ref() != Some(&new_url)
                                            || quality_changed
                                        {
                                            info!(
                                                "Connection changed for {}: {:?} -> {} ({:?})",
                                                source_id, previous_url, new_url, connection_type
                                            );
                                            let _ = sender.output(
                                                ConnectionMonitorOutput::ConnectionChanged {
                                                    source_id: source_id.clone(),
                                                    new_url: new_url.clone(),
                                                    connection_type,
                                                },
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

                                // Calculate next check time based on current quality
                                let current_quality = source
                                    .connection_quality
                                    .as_deref()
                                    .or(previous_quality.as_deref());

                                let next_check = match current_quality {
                                    Some("local") => Instant::now() + Duration::from_secs(300),
                                    Some("remote") => Instant::now() + Duration::from_secs(120),
                                    Some("relay") => Instant::now() + Duration::from_secs(30),
                                    _ => Instant::now() + Duration::from_secs(60),
                                };

                                next_check_times.insert(source_id, next_check);

                                // Small delay between sources to avoid overwhelming
                                tokio::time::sleep(Duration::from_millis(100)).await;
                            }

                            if sources_to_check > 0 {
                                info!(
                                    "Checked {} sources for connection updates",
                                    sources_to_check
                                );
                            }

                            // Update the monitor's next check times and auth status
                            sender
                                .input(ConnectionMonitorInput::UpdateCheckTimes(next_check_times));
                            sender
                                .input(ConnectionMonitorInput::UpdateAuthStatus(last_auth_status));
                        }
                        Err(e) => {
                            warn!("Failed to get sources for connection monitoring: {}", e);
                        }
                    }
                });
            }

            ConnectionMonitorInput::UpdateCheckTimes(times) => {
                self.next_check_times = times;
            }

            ConnectionMonitorInput::UpdateAuthStatus(status) => {
                self.last_auth_status = status;
            }
        }
    }
}

impl ConnectionMonitor {
    /// Calculate next check time based on connection quality
    pub fn calculate_next_check(&self, _source_id: &SourceId, quality: Option<&str>) -> Instant {
        let interval = match quality {
            Some("local") => Duration::from_secs(300), // 5 minutes for local
            Some("remote") => Duration::from_secs(120), // 2 minutes for remote
            Some("relay") => Duration::from_secs(30),  // 30 seconds for relay
            _ => Duration::from_secs(60),              // 1 minute default
        };
        Instant::now() + interval
    }

    /// Check if a source needs checking based on its quality
    pub fn should_check_source(&self, source_id: &SourceId) -> bool {
        self.next_check_times
            .get(source_id)
            .map(|&next_check| Instant::now() >= next_check)
            .unwrap_or(true) // Check if we haven't tracked it yet
    }

    /// Start periodic monitoring of all sources with variable frequency
    pub fn start_monitoring(sender: relm4::Sender<ConnectionMonitorInput>, runtime: Handle) {
        runtime.spawn(async move {
            // Use a shorter base interval to check more frequently
            // Individual sources will be skipped if not due for checking
            let mut interval = tokio::time::interval(Duration::from_secs(10));

            loop {
                interval.tick().await;

                // Send check all sources message
                // The handler will skip sources that aren't due for checking
                let _ = sender.send(ConnectionMonitorInput::CheckAllSources);
            }
        });
    }
}

#[cfg(test)]
#[path = "connection_monitor_tests.rs"]
mod tests;
