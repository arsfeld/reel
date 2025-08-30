pub mod app;
pub mod error;
pub mod main;
pub mod simple_window;
pub mod window;
// pub mod window_controller;  // Temporarily disabled - needs fixes

pub mod bindings;
pub mod controllers;
pub mod delegates;
pub mod dialogs;
pub mod utils;
pub mod views;

pub use app::CocoaApp;
pub use error::{CocoaError, CocoaResult, ErrorHandler};

#[cfg(target_os = "macos")]
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    main::main()
}
