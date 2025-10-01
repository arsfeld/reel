use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "cache_download_queue")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub media_id: String,
    pub source_id: String,
    pub quality: String,
    pub priority: i32,
    pub status: String,
    pub retry_count: i32,
    pub last_retry_at: Option<DateTime>,
    pub created_at: DateTime,
    pub scheduled_for: Option<DateTime>,
    pub expires_at: Option<DateTime>,
    pub user_requested: bool,
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
