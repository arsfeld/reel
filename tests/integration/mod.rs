//! Integration tests for Reel media player
//!
//! These tests verify the complete flow from backend to database:
//! - Authentication with Plex and Jellyfin servers
//! - Library discovery and sync
//! - Media item fetching
//! - Playback progress tracking
//! - Error handling
//!
//! Tests use mockito to simulate server responses, providing fast and reliable
//! integration testing without requiring actual Plex/Jellyfin servers.
//!
//! For E2E testing with real Docker containers, see the `common` module which
//! provides Docker container fixtures using testcontainers.

mod common;
mod fixtures;
mod jellyfin;
mod plex;
