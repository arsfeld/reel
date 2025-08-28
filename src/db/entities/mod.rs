pub mod libraries;
pub mod media_items;
pub mod offline_content;
pub mod playback_progress;
pub mod sources;
pub mod sync_status;

// Re-export entities for convenience
pub use libraries::{ActiveModel as LibraryActiveModel, Entity as Library, Model as LibraryModel};
pub use media_items::{
    ActiveModel as MediaItemActiveModel, Entity as MediaItem, Model as MediaItemModel,
};
pub use offline_content::{
    ActiveModel as OfflineContentActiveModel, Entity as OfflineContent,
    Model as OfflineContentModel,
};
pub use playback_progress::{
    ActiveModel as PlaybackProgressActiveModel, Entity as PlaybackProgress,
    Model as PlaybackProgressModel,
};
pub use sources::{ActiveModel as SourceActiveModel, Entity as Source, Model as SourceModel};
pub use sync_status::{
    ActiveModel as SyncStatusActiveModel, Entity as SyncStatus, Model as SyncStatusModel,
};
