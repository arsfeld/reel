// Module organization for Plex API

mod client;
mod home;
mod library;
mod markers;
pub mod playqueue;
mod progress;
pub mod search;
mod search_impl;
mod streaming;
mod types;

// Re-export the main PlexApi struct, constants, and helper functions
pub use client::{PlexApi, create_standard_headers};
// Re-export PlayQueue types for external use
