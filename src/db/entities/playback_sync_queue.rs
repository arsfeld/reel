use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "playback_sync_queue")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub media_item_id: String,
    pub source_id: i32,
    pub user_id: Option<String>,
    pub change_type: String, // 'progress_update' | 'mark_watched' | 'mark_unwatched'
    pub position_ms: Option<i64>,
    pub completed: Option<bool>,
    pub created_at: DateTime,
    pub last_attempt_at: Option<DateTime>,
    pub attempt_count: i32,
    pub error_message: Option<String>,
    pub status: String, // 'pending' | 'syncing' | 'synced' | 'failed'
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::media_items::Entity",
        from = "Column::MediaItemId",
        to = "super::media_items::Column::Id"
    )]
    MediaItem,
    #[sea_orm(
        belongs_to = "super::sources::Entity",
        from = "Column::SourceId",
        to = "super::sources::Column::Id"
    )]
    Source,
}

impl Related<super::media_items::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MediaItem.def()
    }
}

impl Related<super::sources::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Source.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Change type for playback sync operations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncChangeType {
    ProgressUpdate,
    MarkWatched,
    MarkUnwatched,
}

impl std::fmt::Display for SyncChangeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncChangeType::ProgressUpdate => write!(f, "progress_update"),
            SyncChangeType::MarkWatched => write!(f, "mark_watched"),
            SyncChangeType::MarkUnwatched => write!(f, "mark_unwatched"),
        }
    }
}

impl std::str::FromStr for SyncChangeType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "progress_update" => Ok(SyncChangeType::ProgressUpdate),
            "mark_watched" => Ok(SyncChangeType::MarkWatched),
            "mark_unwatched" => Ok(SyncChangeType::MarkUnwatched),
            _ => Err(format!("Invalid sync change type: {}", s)),
        }
    }
}

/// Sync status for queue items
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackSyncStatus {
    Pending,
    Syncing,
    Synced,
    Failed,
}

impl std::fmt::Display for PlaybackSyncStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlaybackSyncStatus::Pending => write!(f, "pending"),
            PlaybackSyncStatus::Syncing => write!(f, "syncing"),
            PlaybackSyncStatus::Synced => write!(f, "synced"),
            PlaybackSyncStatus::Failed => write!(f, "failed"),
        }
    }
}

impl std::str::FromStr for PlaybackSyncStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(PlaybackSyncStatus::Pending),
            "syncing" => Ok(PlaybackSyncStatus::Syncing),
            "synced" => Ok(PlaybackSyncStatus::Synced),
            "failed" => Ok(PlaybackSyncStatus::Failed),
            _ => Err(format!("Invalid sync status: {}", s)),
        }
    }
}

impl Model {
    /// Get the change type as an enum
    pub fn get_change_type(&self) -> Result<SyncChangeType, String> {
        self.change_type.parse()
    }

    /// Get the status as an enum
    pub fn get_status(&self) -> Result<PlaybackSyncStatus, String> {
        self.status.parse()
    }

    /// Check if this sync item can be retried
    pub fn is_retryable(&self, max_attempts: i32) -> bool {
        self.status == PlaybackSyncStatus::Failed.to_string() && self.attempt_count < max_attempts
    }

    /// Check if this sync item is considered stale (not updated in a while)
    pub fn is_stale(&self, stale_threshold_seconds: i64) -> bool {
        if let Some(last_attempt) = self.last_attempt_at {
            let now = chrono::Utc::now().naive_utc();
            let elapsed = now.signed_duration_since(last_attempt);
            elapsed.num_seconds() > stale_threshold_seconds
        } else {
            false
        }
    }
}
