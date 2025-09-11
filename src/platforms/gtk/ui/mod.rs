pub mod auth_dialog;
pub mod components;
pub mod filters;
pub mod main_window;
pub mod navigation;
pub mod navigation_request;
pub mod page_factory;
pub mod pages;
pub mod preferences_window;
pub mod reactive;
pub mod viewmodels;
pub mod widgets;

// Export the Blueprint-based components
pub use auth_dialog::ReelAuthDialog as AuthDialog;
pub use preferences_window::PreferencesWindow;

// Navigation components are used internally by main_window
