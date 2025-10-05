//! Reel - A modern media player library
//!
//! This library provides the core functionality for the Reel media player,
//! including backends for Plex and Jellyfin, database layer, and services.

pub mod app;
pub mod backends;
pub mod cache;
pub mod config;
pub mod constants;
pub mod core;
pub mod db;
pub mod mapper;
pub mod models;
pub mod player;
pub mod services;
pub mod ui;
pub mod utils;
pub mod workers;

#[cfg(test)]
pub mod test_utils;
