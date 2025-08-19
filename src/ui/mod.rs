pub mod window_blueprint;
pub mod auth_dialog;
pub mod widgets;
pub mod components;
pub mod pages;
pub mod preferences_window;

// Export the Blueprint-based components
pub use window_blueprint::ReelMainWindow;
pub use window_blueprint::ReelMainWindow as MainWindow;
pub use auth_dialog::ReelAuthDialog as AuthDialog;
pub use preferences_window::PreferencesWindow;