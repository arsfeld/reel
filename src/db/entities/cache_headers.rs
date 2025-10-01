use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "cache_headers")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub cache_entry_id: i32,
    pub header_name: String,
    pub header_value: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::cache_entries::Entity",
        from = "Column::CacheEntryId",
        to = "super::cache_entries::Column::Id"
    )]
    CacheEntry,
}

impl Related<super::cache_entries::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CacheEntry.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
