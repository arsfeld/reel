// Module organization for Plex API

mod client;
pub mod errors;
mod home;
mod library;
mod markers;
pub mod playqueue;
mod progress;
pub mod retry;
mod streaming;
mod types;

// Re-export the main PlexApi struct, constants, and helper functions
pub use client::{PlexApi, create_standard_headers};
pub use errors::PlexApiError;
pub use retry::RetryPolicy;
// Re-export PlayQueue types for external use
