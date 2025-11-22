use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Represents different authentication providers for media sources
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum AuthProvider {
    /// Plex account that can discover multiple servers
    PlexAccount {
        id: String,
        username: String,
        email: String,
        #[serde(skip)]
        token: String, // Store in keyring
        refresh_token: Option<String>,
        token_expiry: Option<DateTime<Utc>>,
    },
    /// Direct Jellyfin server connection
    JellyfinAuth {
        id: String,
        server_url: String,
        username: String,
        user_id: String,
        #[serde(skip)]
        access_token: String, // Store in keyring
    },
    /// Network share credentials (SMB, NFS, WebDAV, etc.)
    NetworkCredentials {
        id: String,
        display_name: String,
        auth_type: NetworkAuthType,
        #[serde(skip)]
        credentials: NetworkCredentialData, // Store in keyring
    },
    /// Local files don't need authentication
    LocalFiles { id: String },
}

impl AuthProvider {
    pub fn id(&self) -> &str {
        match self {
            Self::PlexAccount { id, .. } => id,
            Self::JellyfinAuth { id, .. } => id,
            Self::NetworkCredentials { id, .. } => id,
            Self::LocalFiles { id } => id,
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            Self::PlexAccount { username, .. } => username.clone(),
            Self::JellyfinAuth {
                username,
                server_url,
                ..
            } => {
                format!("{} @ {}", username, server_url)
            }
            Self::NetworkCredentials { display_name, .. } => display_name.clone(),
            Self::LocalFiles { .. } => "Local Files".to_string(),
        }
    }

    pub fn provider_type(&self) -> &'static str {
        match self {
            Self::PlexAccount { .. } => "plex",
            Self::JellyfinAuth { .. } => "jellyfin",
            Self::NetworkCredentials { .. } => "network",
            Self::LocalFiles { .. } => "local",
        }
    }

    pub fn needs_refresh(&self) -> bool {
        match self {
            Self::PlexAccount { token_expiry, .. } => {
                if let Some(expiry) = token_expiry {
                    Utc::now() >= *expiry
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Get the authentication token for this provider (if applicable)
    /// Used for connection testing and API requests
    pub fn auth_token(&self) -> Option<&str> {
        match self {
            Self::PlexAccount { token, .. } => Some(token),
            Self::JellyfinAuth { access_token, .. } => Some(access_token),
            Self::NetworkCredentials { .. } | Self::LocalFiles { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NetworkAuthType {
    SMB { domain: Option<String> },
    SFTP { use_key: bool },
    WebDAV,
    NFS,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NetworkCredentialData {
    UsernamePassword {
        username: String,
        #[serde(skip)]
        password: String, // Store in keyring
    },
    SSHKey {
        key_path: PathBuf,
        #[serde(skip)]
        passphrase: Option<String>, // Store in keyring
    },
    Token(#[serde(skip)] String), // Store in keyring
}

impl Default for NetworkCredentialData {
    fn default() -> Self {
        NetworkCredentialData::UsernamePassword {
            username: String::new(),
            password: String::new(),
        }
    }
}

/// Represents a discovered or configured media source
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Source {
    pub id: String,
    pub name: String,
    pub source_type: SourceType,
    pub auth_provider_id: Option<String>,
    pub connection_info: ConnectionInfo,
    pub enabled: bool,
    pub last_sync: Option<DateTime<Utc>>,
    #[serde(default)]
    pub library_count: usize,
    #[serde(default)]
    pub auth_status: crate::models::AuthStatus,
    pub last_auth_check: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SourceType {
    PlexServer {
        machine_id: String,
        owned: bool,
    },
    JellyfinServer,
    NetworkShare {
        path: String,
        share_type: NetworkAuthType,
    },
    LocalFolder {
        path: PathBuf,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectionInfo {
    pub primary_url: Option<String>,
    pub is_online: bool,
    pub last_check: Option<DateTime<Utc>>,
    #[serde(default)]
    pub connection_quality: Option<String>, // "local", "remote", or "relay"
}

impl Source {
    pub fn new(
        id: String,
        name: String,
        source_type: SourceType,
        auth_provider_id: Option<String>,
    ) -> Self {
        Self {
            id,
            name,
            source_type,
            auth_provider_id,
            connection_info: ConnectionInfo {
                primary_url: None,
                is_online: false,
                last_check: None,
                connection_quality: None,
            },
            enabled: true,
            last_sync: None,
            library_count: 0,
            auth_status: crate::models::AuthStatus::Unknown,
            last_auth_check: None,
        }
    }

    pub fn needs_reauthentication(&self) -> bool {
        self.auth_status == crate::models::AuthStatus::AuthRequired
    }

    pub fn is_online(&self) -> bool {
        self.connection_info.is_online
    }

    pub fn source_icon(&self) -> &'static str {
        match &self.source_type {
            SourceType::PlexServer { .. } => "network-server-symbolic",
            SourceType::JellyfinServer => "network-workgroup-symbolic",
            SourceType::NetworkShare { .. } => "folder-remote-symbolic",
            SourceType::LocalFolder { .. } => "folder-symbolic",
        }
    }
}

// Convert database SourceModel to domain Source
impl From<crate::db::entities::SourceModel> for Source {
    fn from(model: crate::db::entities::SourceModel) -> Self {
        let source_type = match model.source_type.as_str() {
            "plex" => SourceType::PlexServer {
                machine_id: model.machine_id.clone().unwrap_or_else(|| model.id.clone()),
                owned: model.is_owned,
            },
            "jellyfin" => SourceType::JellyfinServer,
            "local" => SourceType::LocalFolder {
                path: PathBuf::from(model.connection_url.as_deref().unwrap_or("/")),
            },
            _ => SourceType::JellyfinServer, // Default fallback
        };

        Self {
            id: model.id,
            name: model.name,
            source_type,
            auth_provider_id: model.auth_provider_id,
            connection_info: ConnectionInfo {
                primary_url: model.connection_url,
                is_online: model.is_online,
                last_check: model
                    .last_sync
                    .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)),
                connection_quality: model.connection_quality, // Load from database
            },
            enabled: true, // Database doesn't have this field, default to true
            last_sync: model
                .last_sync
                .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)),
            library_count: 0, // Will be loaded separately
            auth_status: crate::models::AuthStatus::from(model.auth_status),
            last_auth_check: model
                .last_auth_check
                .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)),
        }
    }
}
