pub mod main_window;
pub mod auth_dialog;
pub mod widgets;
pub mod components;
pub mod pages;
pub mod preferences_window;

// Export the Blueprint-based components
pub use main_window::ReelMainWindow;
pub use main_window::ReelMainWindow as MainWindow;
pub use auth_dialog::ReelAuthDialog as AuthDialog;
pub use preferences_window::PreferencesWindow;