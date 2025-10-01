// Module organization for Plex API

mod client;
mod home;
mod library;
pub mod playqueue;
mod progress;
mod streaming;
mod types;

// Re-export the main PlexApi struct, constants, and helper functions
pub use client::{PlexApi, create_standard_headers};
// Re-export PlayQueue types for external use
