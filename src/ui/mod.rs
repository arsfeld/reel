pub mod window_blueprint;
pub mod auth_dialog;
pub mod widgets;
pub mod components;

// Export the Blueprint-based components
pub use window_blueprint::ReelMainWindow;
pub use window_blueprint::ReelMainWindow as MainWindow;
pub use auth_dialog::ReelAuthDialog as AuthDialog;