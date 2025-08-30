use objc2_app_kit::NSAlert;
use objc2_foundation::NSString;
use std::fmt;

/// Error types specific to the Cocoa frontend
#[derive(Debug)]
pub enum CocoaError {
    /// Failed to initialize application
    InitializationFailed(String),

    /// Window creation failed
    WindowCreationFailed(String),

    /// View creation failed
    ViewCreationFailed(String),

    /// Event handling error
    EventHandlingError(String),

    /// Database access error
    DatabaseError(String),

    /// Sync error
    SyncError(String),

    /// Authentication failed
    AuthenticationFailed(String),

    /// User cancelled operation
    UserCancelled,

    /// Invalid URL
    InvalidURL,

    /// Invalid state
    InvalidState(String),

    /// Not implemented
    NotImplemented(String),

    /// ViewModel error
    ViewModelError(String),

    /// Generic error
    Other(String),
}

impl fmt::Display for CocoaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InitializationFailed(msg) => write!(f, "Initialization failed: {}", msg),
            Self::WindowCreationFailed(msg) => write!(f, "Window creation failed: {}", msg),
            Self::ViewCreationFailed(msg) => write!(f, "View creation failed: {}", msg),
            Self::EventHandlingError(msg) => write!(f, "Event handling error: {}", msg),
            Self::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            Self::SyncError(msg) => write!(f, "Sync error: {}", msg),
            Self::AuthenticationFailed(msg) => write!(f, "Authentication failed: {}", msg),
            Self::UserCancelled => write!(f, "User cancelled"),
            Self::InvalidURL => write!(f, "Invalid URL"),
            Self::InvalidState(msg) => write!(f, "Invalid state: {}", msg),
            Self::NotImplemented(msg) => write!(f, "Not implemented: {}", msg),
            Self::ViewModelError(msg) => write!(f, "ViewModel error: {}", msg),
            Self::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for CocoaError {}

impl From<Box<dyn std::error::Error>> for CocoaError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        CocoaError::Other(err.to_string())
    }
}

impl From<String> for CocoaError {
    fn from(err: String) -> Self {
        CocoaError::Other(err)
    }
}

impl From<&str> for CocoaError {
    fn from(err: &str) -> Self {
        CocoaError::Other(err.to_string())
    }
}

impl From<url::ParseError> for CocoaError {
    fn from(_: url::ParseError) -> Self {
        CocoaError::InvalidURL
    }
}

/// Result type for Cocoa operations
pub type CocoaResult<T> = Result<T, CocoaError>;

/// Error handler that can display alerts to the user
pub struct ErrorHandler;

impl ErrorHandler {
    /// Show an error alert to the user
    pub fn show_alert(title: &str, message: &str) {
        // Note: NSAlert requires proper implementation with objc2
        // For now, just log the error - in production we'd show an actual alert
        use tracing::error;
        error!("Alert: {} - {}", title, message);
    }

    /// Handle an error with optional user notification
    pub fn handle_error(error: &CocoaError, show_alert: bool) {
        use tracing::error;

        // Log the error
        error!("{}", error);

        // Show alert if requested
        if show_alert {
            let title = match error {
                CocoaError::InitializationFailed(_) => "Initialization Failed",
                CocoaError::WindowCreationFailed(_) => "Window Error",
                CocoaError::ViewCreationFailed(_) => "View Error",
                CocoaError::EventHandlingError(_) => "Event Error",
                CocoaError::DatabaseError(_) => "Database Error",
                CocoaError::SyncError(_) => "Sync Error",
                CocoaError::AuthenticationFailed(_) => "Authentication Failed",
                CocoaError::UserCancelled => "Cancelled",
                CocoaError::InvalidURL => "Invalid URL",
                CocoaError::InvalidState(_) => "Invalid State",
                CocoaError::NotImplemented(_) => "Not Implemented",
                CocoaError::ViewModelError(_) => "ViewModel Error",
                CocoaError::Other(_) => "Error",
            };

            Self::show_alert(title, &error.to_string());
        }
    }

    /// Create an error handler for async operations
    pub fn async_handler() -> impl Fn(CocoaError) {
        move |error| {
            Self::handle_error(&error, false);
        }
    }
}

/// Macro for error handling in Cocoa code
#[macro_export]
macro_rules! cocoa_try {
    ($expr:expr) => {
        match $expr {
            Ok(val) => val,
            Err(e) => {
                use $crate::platforms::cocoa::error::ErrorHandler;
                let cocoa_err = $crate::platforms::cocoa::error::CocoaError::from(e);
                ErrorHandler::handle_error(&cocoa_err, false);
                return Err(cocoa_err);
            }
        }
    };
    ($expr:expr, $alert:expr) => {
        match $expr {
            Ok(val) => val,
            Err(e) => {
                use $crate::platforms::cocoa::error::ErrorHandler;
                let cocoa_err = $crate::platforms::cocoa::error::CocoaError::from(e);
                ErrorHandler::handle_error(&cocoa_err, $alert);
                return Err(cocoa_err);
            }
        }
    };
}
