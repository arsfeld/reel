pub mod auth_dialog;
pub mod components;
pub mod filters;
pub mod main_window;
pub mod navigation;
pub mod navigation_request;
pub mod pages;
pub mod preferences_window;
pub mod reactive;
pub mod reactive_bindings;
pub mod viewmodels;
pub mod widgets;

// Export the Blueprint-based components
pub use auth_dialog::ReelAuthDialog as AuthDialog;
pub use main_window::ReelMainWindow as MainWindow;
pub use preferences_window::PreferencesWindow;

// Export navigation components
pub use navigation::{NavigationManager, NavigationPage, NavigationState};
pub use navigation_request::NavigationRequest;
