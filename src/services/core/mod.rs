/// Stateless service modules for Relm4 architecture
/// These are pure functions that operate on data without maintaining state
pub mod auth;
pub mod backend;
pub mod media;
pub mod playback;
pub mod sync;

pub use auth::AuthService;
pub use backend::BackendService;
pub use media::MediaService;
pub use playback::PlaybackService;
pub use sync::{SyncProgress, SyncResult, SyncService, SyncStatus};
