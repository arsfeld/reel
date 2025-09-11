use super::{Property, PropertySubscriber, ViewModel};
use crate::events::{DatabaseEvent, EventBus, EventFilter, EventPayload, EventType};
use crate::models::{AuthProvider, Source};
use crate::services::{AuthManager, DataService};
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

/// Authentication states that drive the UI
#[derive(Debug, Clone, PartialEq)]
pub enum AuthenticationState {
    Idle,
    RequestingPin,
    WaitingForUser {
        pin: String,
        elapsed_seconds: u64,
    },
    ValidatingToken,
    DiscoveringServers {
        found: usize,
    },
    TestingConnections {
        current: usize,
        total: usize,
    },
    CreatingAccount,
    LoadingUserInfo,
    StartingSyncForSources {
        sources: Vec<String>,
    },
    Complete {
        provider_id: String,
        sources: Vec<Source>,
    },
    Error {
        error: String,
        can_retry: bool,
    },
}

/// Authentication progress for different backend types
#[derive(Debug, Clone, PartialEq)]
pub enum BackendAuthProgress {
    Plex(PlexAuthProgress),
    Jellyfin(JellyfinAuthProgress),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlexAuthProgress {
    RequestingPin,
    WaitingForUser { pin: String, elapsed_seconds: u64 },
    ValidatingToken,
    DiscoveringServers { found: usize },
    TestingConnections { current: usize, total: usize },
    CreatingAccount,
    Complete { sources: Vec<Source> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum JellyfinAuthProgress {
    ValidatingCredentials,
    TestingConnection,
    CreatingAccount,
    Complete { source: Source },
}

/// Error types that determine recovery options
#[derive(Debug, Clone, PartialEq)]
pub enum AuthenticationError {
    NetworkError { can_retry: bool },
    InvalidCredentials { field: String },
    ServerUnavailable { server_name: String },
    TokenExpired { refresh_available: bool },
    UserCancelled,
    Timeout,
    UnknownError { message: String },
}

/// ViewModel for authentication dialog and process management
pub struct AuthenticationViewModel {
    auth_manager: Arc<AuthManager>,
    data_service: Arc<DataService>,

    // Observable Properties
    authentication_state: Property<AuthenticationState>,
    progress: Property<Option<BackendAuthProgress>>,
    error: Property<Option<AuthenticationError>>,
    is_cancellable: Property<bool>,

    // Plex-specific properties
    current_pin: Property<Option<String>>,
    pin_elapsed_time: Property<u64>,

    // Jellyfin-specific properties
    jellyfin_credentials: Property<Option<(String, String, String)>>, // url, username, password

    // Results
    discovered_sources: Property<Vec<Source>>,
    created_provider: Property<Option<AuthProvider>>,

    // Internal state
    event_bus: Option<Arc<EventBus>>,
    cancellation_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl AuthenticationViewModel {
    pub fn new(auth_manager: Arc<AuthManager>, data_service: Arc<DataService>) -> Self {
        Self {
            auth_manager,
            data_service,
            authentication_state: Property::new(AuthenticationState::Idle, "authentication_state"),
            progress: Property::new(None, "progress"),
            error: Property::new(None, "error"),
            is_cancellable: Property::new(false, "is_cancellable"),
            current_pin: Property::new(None, "current_pin"),
            pin_elapsed_time: Property::new(0, "pin_elapsed_time"),
            jellyfin_credentials: Property::new(None, "jellyfin_credentials"),
            discovered_sources: Property::new(Vec::new(), "discovered_sources"),
            created_provider: Property::new(None, "created_provider"),
            event_bus: None,
            cancellation_handle: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    /// Start Plex authentication process
    pub async fn start_plex_authentication(&self) -> Result<()> {
        info!("Starting Plex authentication");

        self.authentication_state
            .set(AuthenticationState::RequestingPin)
            .await;
        self.progress
            .set(Some(BackendAuthProgress::Plex(
                PlexAuthProgress::RequestingPin,
            )))
            .await;
        self.error.set(None).await;
        self.is_cancellable.set(true).await;

        let vm = self.clone();
        let handle = tokio::spawn(async move {
            if let Err(e) = vm.plex_authentication_flow().await {
                let error = match e.to_string().as_str() {
                    msg if msg.contains("network") => {
                        AuthenticationError::NetworkError { can_retry: true }
                    }
                    msg if msg.contains("timeout") => AuthenticationError::Timeout,
                    msg if msg.contains("cancelled") => AuthenticationError::UserCancelled,
                    _ => AuthenticationError::UnknownError {
                        message: e.to_string(),
                    },
                };

                let _ = vm.error.set(Some(error.clone())).await;
                let _ = vm
                    .authentication_state
                    .set(AuthenticationState::Error {
                        error: e.to_string(),
                        can_retry: !matches!(error, AuthenticationError::UserCancelled),
                    })
                    .await;
            }
        });

        *self.cancellation_handle.lock().await = Some(handle);
        Ok(())
    }

    async fn plex_authentication_flow(&self) -> Result<()> {
        use crate::backends::plex::PlexAuth;

        // Step 1: Request PIN
        let pin = PlexAuth::get_pin().await?;
        self.current_pin.set(Some(pin.code.clone())).await;
        self.progress
            .set(Some(BackendAuthProgress::Plex(
                PlexAuthProgress::WaitingForUser {
                    pin: pin.code.clone(),
                    elapsed_seconds: 0,
                },
            )))
            .await;
        self.authentication_state
            .set(AuthenticationState::WaitingForUser {
                pin: pin.code.clone(),
                elapsed_seconds: 0,
            })
            .await;

        // Start elapsed time counter
        let vm = self.clone();
        let _pin_id = pin.id.clone();
        tokio::spawn(async move {
            let mut elapsed = 0u64;
            let mut interval = tokio::time::interval(Duration::from_secs(1));

            loop {
                interval.tick().await;
                elapsed += 1;

                if matches!(
                    vm.authentication_state.get().await,
                    AuthenticationState::WaitingForUser { .. }
                ) {
                    let _ = vm.pin_elapsed_time.set(elapsed).await;
                    let _ = vm
                        .progress
                        .set(Some(BackendAuthProgress::Plex(
                            PlexAuthProgress::WaitingForUser {
                                pin: pin.code.clone(),
                                elapsed_seconds: elapsed,
                            },
                        )))
                        .await;
                } else {
                    break;
                }
            }
        });

        // Step 2: Poll for token
        self.authentication_state
            .set(AuthenticationState::ValidatingToken)
            .await;
        let token = PlexAuth::poll_for_token(&pin.id, None).await?;

        // Step 3: Discover servers
        self.authentication_state
            .set(AuthenticationState::DiscoveringServers { found: 0 })
            .await;
        self.progress
            .set(Some(BackendAuthProgress::Plex(
                PlexAuthProgress::DiscoveringServers { found: 0 },
            )))
            .await;

        let servers = PlexAuth::discover_servers(&token).await?;
        self.authentication_state
            .set(AuthenticationState::DiscoveringServers {
                found: servers.len(),
            })
            .await;

        // Step 4: Test connections
        self.authentication_state
            .set(AuthenticationState::TestingConnections {
                current: 0,
                total: servers.len(),
            })
            .await;
        self.progress
            .set(Some(BackendAuthProgress::Plex(
                PlexAuthProgress::TestingConnections {
                    current: 0,
                    total: servers.len(),
                },
            )))
            .await;

        // For now, skip actual connection testing and use first server
        // TODO: Implement proper connection testing with progress updates

        // Step 5: Create account
        self.authentication_state
            .set(AuthenticationState::CreatingAccount)
            .await;
        self.progress
            .set(Some(BackendAuthProgress::Plex(
                PlexAuthProgress::CreatingAccount,
            )))
            .await;

        let (provider_id, sources) = self.auth_manager.add_plex_account(&token).await?;

        self.discovered_sources.set(sources.clone()).await;

        if let Some(provider) = self.auth_manager.get_provider(&provider_id).await {
            self.created_provider.set(Some(provider)).await;
        }

        // Step 6: Complete
        self.authentication_state
            .set(AuthenticationState::Complete {
                provider_id,
                sources: sources.clone(),
            })
            .await;
        self.progress
            .set(Some(BackendAuthProgress::Plex(
                PlexAuthProgress::Complete { sources },
            )))
            .await;
        self.is_cancellable.set(false).await;

        // Emit completion event
        if let Some(event_bus) = &self.event_bus {
            let event = DatabaseEvent::new(
                EventType::UserAuthenticated,
                EventPayload::User {
                    user_id: "plex_user".to_string(), // TODO: Get actual user ID
                    action: "plex_authentication_completed".to_string(),
                },
            );
            let _ = event_bus.publish(event).await;
        }

        Ok(())
    }

    /// Start Jellyfin authentication process
    pub async fn start_jellyfin_authentication(
        &self,
        url: String,
        username: String,
        password: String,
    ) -> Result<()> {
        info!("Starting Jellyfin authentication for {}", url);

        self.authentication_state
            .set(AuthenticationState::RequestingPin)
            .await; // Reuse for "starting"
        self.progress
            .set(Some(BackendAuthProgress::Jellyfin(
                JellyfinAuthProgress::ValidatingCredentials,
            )))
            .await;
        self.jellyfin_credentials
            .set(Some((url.clone(), username.clone(), password.clone())))
            .await;
        self.error.set(None).await;
        self.is_cancellable.set(true).await;

        let vm = self.clone();
        let url_clone = url.clone();
        let handle = tokio::spawn(async move {
            if let Err(e) = vm
                .jellyfin_authentication_flow(url, username, password)
                .await
            {
                let error = match e.to_string().as_str() {
                    msg if msg.contains("network") => {
                        AuthenticationError::NetworkError { can_retry: true }
                    }
                    msg if msg.contains("authentication") || msg.contains("unauthorized") => {
                        AuthenticationError::InvalidCredentials {
                            field: "credentials".to_string(),
                        }
                    }
                    msg if msg.contains("server") => AuthenticationError::ServerUnavailable {
                        server_name: url_clone,
                    },
                    _ => AuthenticationError::UnknownError {
                        message: e.to_string(),
                    },
                };

                let _ = vm.error.set(Some(error.clone())).await;
                let _ = vm
                    .authentication_state
                    .set(AuthenticationState::Error {
                        error: e.to_string(),
                        can_retry: !matches!(error, AuthenticationError::UserCancelled),
                    })
                    .await;
            }
        });

        *self.cancellation_handle.lock().await = Some(handle);
        Ok(())
    }

    async fn jellyfin_authentication_flow(
        &self,
        url: String,
        username: String,
        password: String,
    ) -> Result<()> {
        use crate::backends::jellyfin::JellyfinBackend;

        // Step 1: Test connection
        self.authentication_state
            .set(AuthenticationState::ValidatingToken)
            .await; // Reuse
        self.progress
            .set(Some(BackendAuthProgress::Jellyfin(
                JellyfinAuthProgress::TestingConnection,
            )))
            .await;

        let temp_backend = JellyfinBackend::new();
        temp_backend
            .authenticate_with_credentials(&url, &username, &password)
            .await?;

        // Step 2: Get credentials
        let (access_token, user_id) = temp_backend
            .get_credentials()
            .await
            .ok_or_else(|| anyhow::anyhow!("Failed to retrieve credentials"))?;

        // Step 3: Create account
        self.authentication_state
            .set(AuthenticationState::CreatingAccount)
            .await;
        self.progress
            .set(Some(BackendAuthProgress::Jellyfin(
                JellyfinAuthProgress::CreatingAccount,
            )))
            .await;

        let (provider_id, source) = self
            .auth_manager
            .add_jellyfin_auth(&url, &username, &password, &access_token, &user_id)
            .await?;

        self.discovered_sources.set(vec![source.clone()]).await;

        if let Some(provider) = self.auth_manager.get_provider(&provider_id).await {
            self.created_provider.set(Some(provider)).await;
        }

        // Step 4: Complete
        self.authentication_state
            .set(AuthenticationState::Complete {
                provider_id,
                sources: vec![source.clone()],
            })
            .await;
        self.progress
            .set(Some(BackendAuthProgress::Jellyfin(
                JellyfinAuthProgress::Complete { source },
            )))
            .await;
        self.is_cancellable.set(false).await;

        // Emit completion event
        if let Some(event_bus) = &self.event_bus {
            let event = DatabaseEvent::new(
                EventType::UserAuthenticated,
                EventPayload::User {
                    user_id: username,
                    action: "jellyfin_authentication_completed".to_string(),
                },
            );
            let _ = event_bus.publish(event).await;
        }

        Ok(())
    }

    /// Cancel current authentication process
    pub async fn cancel_authentication(&self) -> Result<()> {
        info!("Cancelling authentication");

        if let Some(handle) = self.cancellation_handle.lock().await.take() {
            handle.abort();
        }

        self.authentication_state
            .set(AuthenticationState::Error {
                error: "Authentication cancelled".to_string(),
                can_retry: true,
            })
            .await;

        self.error
            .set(Some(AuthenticationError::UserCancelled))
            .await;
        self.is_cancellable.set(false).await;

        Ok(())
    }

    /// Retry authentication after error
    pub async fn retry_authentication(&self) -> Result<()> {
        info!("Retrying authentication");

        // Clear error state
        self.error.set(None).await;
        self.authentication_state
            .set(AuthenticationState::Idle)
            .await;

        // Restart based on what was being attempted
        if let Some((url, username, password)) = self.jellyfin_credentials.get().await {
            self.start_jellyfin_authentication(url, username, password)
                .await
        } else {
            self.start_plex_authentication().await
        }
    }

    /// Reset to initial state
    pub async fn reset(&self) {
        if let Some(handle) = self.cancellation_handle.lock().await.take() {
            handle.abort();
        }

        self.authentication_state
            .set(AuthenticationState::Idle)
            .await;
        self.progress.set(None).await;
        self.error.set(None).await;
        self.is_cancellable.set(false).await;
        self.current_pin.set(None).await;
        self.pin_elapsed_time.set(0).await;
        self.jellyfin_credentials.set(None).await;
        self.discovered_sources.set(Vec::new()).await;
        self.created_provider.set(None).await;
    }

    // Property getters
    pub fn authentication_state(&self) -> &Property<AuthenticationState> {
        &self.authentication_state
    }

    pub fn progress(&self) -> &Property<Option<BackendAuthProgress>> {
        &self.progress
    }

    pub fn error(&self) -> &Property<Option<AuthenticationError>> {
        &self.error
    }

    pub fn is_cancellable(&self) -> &Property<bool> {
        &self.is_cancellable
    }

    pub fn current_pin(&self) -> &Property<Option<String>> {
        &self.current_pin
    }

    pub fn pin_elapsed_time(&self) -> &Property<u64> {
        &self.pin_elapsed_time
    }

    pub fn discovered_sources(&self) -> &Property<Vec<Source>> {
        &self.discovered_sources
    }

    pub fn created_provider(&self) -> &Property<Option<AuthProvider>> {
        &self.created_provider
    }
}

#[async_trait::async_trait]
impl ViewModel for AuthenticationViewModel {
    async fn initialize(&self, event_bus: Arc<EventBus>) {
        // Store event bus reference
        let mut vm = self.clone();
        vm.event_bus = Some(event_bus.clone());

        // Subscribe to relevant events
        let filter = EventFilter::new().with_types(vec![
            EventType::UserAuthenticated,
            EventType::UserLoggedOut,
            EventType::SourceAdded,
        ]);

        let mut subscriber = event_bus.subscribe_filtered(filter);
        let vm_clone = vm.clone();

        tokio::spawn(async move {
            while let Ok(event) = subscriber.recv().await {
                vm_clone.handle_event(event).await;
            }
        });
    }

    fn subscribe_to_property(&self, property_name: &str) -> Option<PropertySubscriber> {
        match property_name {
            "authentication_state" => Some(self.authentication_state.subscribe()),
            "progress" => Some(self.progress.subscribe()),
            "error" => Some(self.error.subscribe()),
            "is_cancellable" => Some(self.is_cancellable.subscribe()),
            "current_pin" => Some(self.current_pin.subscribe()),
            "pin_elapsed_time" => Some(self.pin_elapsed_time.subscribe()),
            "discovered_sources" => Some(self.discovered_sources.subscribe()),
            "created_provider" => Some(self.created_provider.subscribe()),
            _ => None,
        }
    }

    async fn refresh(&self) {
        // Reset authentication state on refresh
        self.reset().await;
    }
}

impl AuthenticationViewModel {
    async fn handle_event(&self, event: DatabaseEvent) {
        match event.event_type {
            EventType::UserAuthenticated => {
                info!("User authenticated event received");
                // Could trigger UI updates or cleanup
            }
            EventType::SourceAdded => {
                info!("Source added event received");
                // Could update discovered sources if relevant
            }
            _ => {}
        }
    }
}

impl Clone for AuthenticationViewModel {
    fn clone(&self) -> Self {
        Self {
            auth_manager: self.auth_manager.clone(),
            data_service: self.data_service.clone(),
            authentication_state: self.authentication_state.clone(),
            progress: self.progress.clone(),
            error: self.error.clone(),
            is_cancellable: self.is_cancellable.clone(),
            current_pin: self.current_pin.clone(),
            pin_elapsed_time: self.pin_elapsed_time.clone(),
            jellyfin_credentials: self.jellyfin_credentials.clone(),
            discovered_sources: self.discovered_sources.clone(),
            created_provider: self.created_provider.clone(),
            event_bus: self.event_bus.clone(),
            cancellation_handle: self.cancellation_handle.clone(),
        }
    }
}
