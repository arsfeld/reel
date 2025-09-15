use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "libraries")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub source_id: String,
    pub title: String,
    pub library_type: String, // 'movies', 'shows', 'music', 'photos', 'mixed'
    pub icon: Option<String>,
    pub item_count: i32,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::sources::Entity",
        from = "Column::SourceId",
        to = "super::sources::Column::Id"
    )]
    Source,
    #[sea_orm(has_many = "super::media_items::Entity")]
    MediaItems,
}

impl Related<super::sources::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Source.def()
    }
}

impl Related<super::media_items::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MediaItems.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// Library type enum for type safety
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LibraryType {
    Movies,
    Shows,
    Music,
    Photos,
    Mixed,
}

impl LibraryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            LibraryType::Movies => "movies",
            LibraryType::Shows => "shows",
            LibraryType::Music => "music",
            LibraryType::Photos => "photos",
            LibraryType::Mixed => "mixed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "movies" => Some(LibraryType::Movies),
            "shows" => Some(LibraryType::Shows),
            "music" => Some(LibraryType::Music),
            "photos" => Some(LibraryType::Photos),
            "mixed" => Some(LibraryType::Mixed),
            _ => None,
        }
    }
}

impl Model {
    pub fn get_library_type(&self) -> Option<LibraryType> {
        LibraryType::from_str(&self.library_type)
    }
}

/// Convert database Model to domain Library
impl TryFrom<Model> for crate::models::Library {
    type Error = anyhow::Error;

    fn try_from(model: Model) -> Result<Self, Self::Error> {
        let library_type = match model.library_type.as_str() {
            "movies" => crate::models::LibraryType::Movies,
            "shows" => crate::models::LibraryType::Shows,
            "music" => crate::models::LibraryType::Music,
            "photos" => crate::models::LibraryType::Photos,
            "mixed" => crate::models::LibraryType::Mixed,
            _ => crate::models::LibraryType::Mixed,
        };

        Ok(crate::models::Library {
            id: model.id,
            title: model.title,
            library_type,
            icon: model.icon,
            item_count: model.item_count,
        })
    }
}
