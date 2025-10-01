use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "cache_statistics")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub total_size: i64,
    pub file_count: i32,
    pub max_size_bytes: i64,
    pub max_file_count: i32,
    pub hit_count: i64,
    pub miss_count: i64,
    pub bytes_served: i64,
    pub bytes_downloaded: i64,
    pub last_cleanup_at: Option<DateTime>,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
