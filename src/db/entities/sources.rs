use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sources")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: String,
    pub source_type: String, // 'plex', 'jellyfin', 'local'
    pub auth_provider_id: Option<String>,
    pub connection_url: Option<String>,
    pub connections: Option<serde_json::Value>, // JSON array of all discovered connections
    pub machine_id: Option<String>,             // Plex machine identifier
    pub is_owned: bool,                         // Whether this is an owned Plex server
    pub is_online: bool,
    pub last_sync: Option<DateTime>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::libraries::Entity")]
    Libraries,
    #[sea_orm(has_many = "super::media_items::Entity")]
    MediaItems,
    #[sea_orm(has_many = "super::sync_status::Entity")]
    SyncStatuses,
}

impl Related<super::libraries::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Libraries.def()
    }
}

impl Related<super::media_items::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MediaItems.def()
    }
}

impl Related<super::sync_status::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SyncStatuses.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// Custom methods for Source entity
impl Model {
    pub fn is_plex(&self) -> bool {
        self.source_type == "plex"
    }

    pub fn is_jellyfin(&self) -> bool {
        self.source_type == "jellyfin"
    }

    pub fn is_local(&self) -> bool {
        self.source_type == "local"
    }
}
