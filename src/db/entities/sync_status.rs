use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sync_status")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub source_id: String,
    pub sync_type: String, // 'full', 'incremental', 'library', 'media'
    pub status: String,    // 'pending', 'running', 'completed', 'failed'
    pub started_at: Option<DateTime>,
    pub completed_at: Option<DateTime>,
    pub items_synced: i32,
    pub error_message: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::sources::Entity",
        from = "Column::SourceId",
        to = "super::sources::Column::Id"
    )]
    Source,
}

impl Related<super::sources::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Source.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// Sync type enum
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SyncType {
    Full,
    Incremental,
    Library(String),
    Media(String),
}

impl SyncType {
    pub fn as_str(&self) -> String {
        match self {
            SyncType::Full => "full".to_string(),
            SyncType::Incremental => "incremental".to_string(),
            SyncType::Library(id) => format!("library:{}", id),
            SyncType::Media(id) => format!("media:{}", id),
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        if s == "full" {
            Some(SyncType::Full)
        } else if s == "incremental" {
            Some(SyncType::Incremental)
        } else if let Some(id) = s.strip_prefix("library:") {
            Some(SyncType::Library(id.to_string()))
        } else if let Some(id) = s.strip_prefix("media:") {
            Some(SyncType::Media(id.to_string()))
        } else {
            None
        }
    }
}

// Sync status enum
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SyncStatusType {
    Pending,
    Running,
    Completed,
    Failed,
}

impl SyncStatusType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SyncStatusType::Pending => "pending",
            SyncStatusType::Running => "running",
            SyncStatusType::Completed => "completed",
            SyncStatusType::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(SyncStatusType::Pending),
            "running" => Some(SyncStatusType::Running),
            "completed" => Some(SyncStatusType::Completed),
            "failed" => Some(SyncStatusType::Failed),
            _ => None,
        }
    }
}

impl Model {
    pub fn get_sync_type(&self) -> Option<SyncType> {
        SyncType::from_str(&self.sync_type)
    }

    pub fn get_status(&self) -> Option<SyncStatusType> {
        SyncStatusType::from_str(&self.status)
    }

    pub fn is_running(&self) -> bool {
        self.status == "running"
    }

    pub fn is_completed(&self) -> bool {
        self.status == "completed"
    }

    pub fn is_failed(&self) -> bool {
        self.status == "failed"
    }

    pub fn get_duration(&self) -> Option<std::time::Duration> {
        match (self.started_at, self.completed_at) {
            (Some(start), Some(end)) => {
                let duration_secs = (end.timestamp() - start.timestamp()).max(0) as u64;
                Some(std::time::Duration::from_secs(duration_secs))
            }
            _ => None,
        }
    }
}
