use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "auth_tokens")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,

    pub source_id: String,

    pub token_type: String,

    #[sea_orm(column_type = "Text")]
    pub token: String,

    pub created_at: DateTime,
    pub updated_at: DateTime,

    pub expires_at: Option<DateTime>,
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
