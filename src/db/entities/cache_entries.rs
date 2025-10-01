use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "cache_entries")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub source_id: String,
    pub media_id: String,
    pub quality: String,
    pub original_url: String,
    pub file_path: String,
    pub file_size: i64,
    pub expected_total_size: Option<i64>,
    pub downloaded_bytes: i64,
    pub is_complete: bool,
    pub priority: i32,
    pub created_at: DateTime,
    pub last_accessed: DateTime,
    pub last_modified: DateTime,
    pub access_count: i64,
    pub mime_type: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub container: Option<String>,
    pub resolution_width: Option<i32>,
    pub resolution_height: Option<i32>,
    pub bitrate: Option<i64>,
    pub duration_secs: Option<f64>,
    pub etag: Option<String>,
    pub expires_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::media_items::Entity",
        from = "Column::MediaId",
        to = "super::media_items::Column::Id"
    )]
    MediaItem,
    #[sea_orm(
        belongs_to = "super::sources::Entity",
        from = "Column::SourceId",
        to = "super::sources::Column::Id"
    )]
    Source,
    #[sea_orm(has_many = "super::cache_chunks::Entity")]
    CacheChunks,
    #[sea_orm(has_many = "super::cache_headers::Entity")]
    CacheHeaders,
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

impl Related<super::cache_chunks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CacheChunks.def()
    }
}

impl Related<super::cache_headers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CacheHeaders.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
