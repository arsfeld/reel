use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "cache_quality_variants")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub media_id: String,
    pub source_id: String,
    pub quality: String,
    pub resolution_width: Option<i32>,
    pub resolution_height: Option<i32>,
    pub bitrate: Option<i64>,
    pub file_size: Option<i64>,
    pub container: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub stream_url: Option<String>,
    pub is_default: bool,
    pub discovered_at: DateTime,
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
