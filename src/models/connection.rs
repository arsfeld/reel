use serde::{Deserialize, Serialize};

/// Represents a discovered connection to a media server
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerConnection {
    pub uri: String,
    pub protocol: String, // "http" or "https"
    pub address: String,  // IP address or hostname
    pub port: u32,
    pub local: bool,   // Is this a local network connection?
    pub relay: bool,   // Is this a Plex relay connection?
    pub priority: i32, // Lower number = higher priority
    pub is_available: bool,
    pub response_time_ms: Option<u64>, // Response time from last health check
}

impl ServerConnection {
    /// Calculate priority score for connection selection
    /// Lower score = better connection
    pub fn priority_score(&self) -> i32 {
        let mut score = self.priority;

        // Prefer local connections
        if self.local {
            score -= 1000;
        }

        // Avoid relay connections
        if self.relay {
            score += 500;
        }

        // Prefer available connections
        if !self.is_available {
            score += 10000;
        }

        // Factor in response time if available
        if let Some(response_time) = self.response_time_ms {
            score += (response_time / 100) as i32;
        }

        score
    }

    /// Check if this is a local network connection
    pub fn is_local_network(&self) -> bool {
        self.local
            || self.address.starts_with("192.168.")
            || self.address.starts_with("10.")
            || self.address.starts_with("172.")
            || self.address == "localhost"
            || self.address == "127.0.0.1"
    }
}

/// Collection of server connections with selection logic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConnections {
    pub connections: Vec<ServerConnection>,
    pub last_check: Option<chrono::DateTime<chrono::Utc>>,
}

impl ServerConnections {
    pub fn new(connections: Vec<ServerConnection>) -> Self {
        Self {
            connections,
            last_check: None,
        }
    }

    /// Get the best available connection
    pub fn best_connection(&self) -> Option<&ServerConnection> {
        self.connections
            .iter()
            .filter(|c| c.is_available)
            .min_by_key(|c| c.priority_score())
    }

    /// Get all local connections
    pub fn local_connections(&self) -> Vec<&ServerConnection> {
        self.connections
            .iter()
            .filter(|c| c.is_local_network())
            .collect()
    }

    /// Get all remote connections (non-relay)
    pub fn remote_connections(&self) -> Vec<&ServerConnection> {
        self.connections
            .iter()
            .filter(|c| !c.is_local_network() && !c.relay)
            .collect()
    }

    /// Get relay connections
    pub fn relay_connections(&self) -> Vec<&ServerConnection> {
        self.connections.iter().filter(|c| c.relay).collect()
    }
}
