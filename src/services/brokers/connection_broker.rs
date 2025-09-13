use tracing::debug;

use crate::models::SourceId;

/// Messages for connection status updates
#[derive(Debug, Clone)]
pub enum ConnectionMessage {
    /// Connection status changed
    StatusChanged {
        source_id: SourceId,
        is_connected: bool,
    },
    /// Connection test result
    TestResult {
        source_id: SourceId,
        is_connected: bool,
        error: Option<String>,
    },
    /// Authentication result
    AuthResult {
        source_id: SourceId,
        success: bool,
        error: Option<String>,
    },
    /// Source added
    SourceAdded { source_id: SourceId },
    /// Source removed
    SourceRemoved { source_id: SourceId },
}

// For now, we'll use these message types directly in components
// Components can create their own relm4::Sender/Receiver channels as needed

/// Convenience functions for logging connection operations
pub fn log_connection_status_changed(source_id: SourceId, is_connected: bool) {
    debug!(
        "Connection status changed: source={}, connected={}",
        source_id, is_connected
    );
}

pub fn log_connection_test_result(source_id: SourceId, is_connected: bool, error: Option<&str>) {
    if let Some(err) = error {
        debug!(
            "Connection test failed: source={}, error={}",
            source_id, err
        );
    } else {
        debug!(
            "Connection test: source={}, connected={}",
            source_id, is_connected
        );
    }
}

pub fn log_auth_result(source_id: SourceId, success: bool, error: Option<&str>) {
    if let Some(err) = error {
        debug!("Authentication failed: source={}, error={}", source_id, err);
    } else {
        debug!("Authentication: source={}, success={}", source_id, success);
    }
}

pub fn log_source_added(source_id: SourceId) {
    debug!("Source added: source={}", source_id);
}

pub fn log_source_removed(source_id: SourceId) {
    debug!("Source removed: source={}", source_id);
}
