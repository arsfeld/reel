use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "playback_progress")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub media_id: String,
    pub user_id: Option<String>,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub watched: bool,
    pub view_count: i32,
    pub last_watched_at: Option<DateTime>,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::media_items::Entity",
        from = "Column::MediaId",
        to = "super::media_items::Column::Id"
    )]
    MediaItem,
}

impl Related<super::media_items::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MediaItem.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Get progress as percentage (0.0 to 1.0)
    pub fn get_progress_percentage(&self) -> f32 {
        if self.duration_ms > 0 {
            (self.position_ms as f32 / self.duration_ms as f32).min(1.0)
        } else {
            0.0
        }
    }

    /// Check if media should be considered "in progress"
    pub fn is_in_progress(&self) -> bool {
        !self.watched && self.position_ms > 0
    }

    /// Check if media is near completion (>90%)
    pub fn is_near_completion(&self) -> bool {
        self.get_progress_percentage() > 0.9
    }

    /// Convert position to Duration
    pub fn get_position_duration(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.position_ms as u64)
    }

    /// Convert duration to Duration
    pub fn get_duration(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.duration_ms as u64)
    }
}
