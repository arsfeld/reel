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
// State module removed in Relm4 migration - components manage their own state
// mod state;
mod utils;
