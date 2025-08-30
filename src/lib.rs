// Library entry for FFI (macOS bridge) and shared code
// This coexists with the GTK binary in src/main.rs

#![allow(clippy::result_large_err)]

mod backends;
mod config;
mod constants;
mod core;
mod db;
mod events;
mod models;
mod platforms;
mod player;
mod services;
mod state;
mod utils;

// Expose Swift bridge APIs when the feature is enabled
#[cfg(feature = "swift")]
pub use platforms::macos::bridge::*;
