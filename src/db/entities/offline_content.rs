use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "offline_content")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub media_id: String,
    pub file_path: String,
    pub file_size_bytes: Option<i64>,
    pub quality: Option<String>,
    pub downloaded_at: DateTime,
    pub last_accessed: Option<DateTime>,
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

// Quality preset enum
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QualityPreset {
    Low,      // 480p
    Medium,   // 720p
    High,     // 1080p
    Ultra,    // 4K
    Original, // Original quality
}

impl QualityPreset {
    pub fn as_str(&self) -> &'static str {
        match self {
            QualityPreset::Low => "low",
            QualityPreset::Medium => "medium",
            QualityPreset::High => "high",
            QualityPreset::Ultra => "ultra",
            QualityPreset::Original => "original",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "low" => Some(QualityPreset::Low),
            "medium" => Some(QualityPreset::Medium),
            "high" => Some(QualityPreset::High),
            "ultra" => Some(QualityPreset::Ultra),
            "original" => Some(QualityPreset::Original),
            _ => None,
        }
    }

    pub fn get_max_resolution(&self) -> (u32, u32) {
        match self {
            QualityPreset::Low => (854, 480),
            QualityPreset::Medium => (1280, 720),
            QualityPreset::High => (1920, 1080),
            QualityPreset::Ultra => (3840, 2160),
            QualityPreset::Original => (0, 0), // No limit
        }
    }
}

impl Model {
    pub fn get_quality_preset(&self) -> Option<QualityPreset> {
        self.quality
            .as_ref()
            .and_then(|q| QualityPreset::from_str(q))
    }

    /// Get file size in MB
    pub fn get_size_mb(&self) -> f64 {
        self.file_size_bytes
            .map(|bytes| bytes as f64 / (1024.0 * 1024.0))
            .unwrap_or(0.0)
    }

    /// Check if file exists on disk
    pub fn file_exists(&self) -> bool {
        std::path::Path::new(&self.file_path).exists()
    }

    /// Update last accessed time
    pub async fn touch(&mut self) -> Result<(), DbErr> {
        self.last_accessed = Some(chrono::Utc::now().naive_utc());
        Ok(())
    }

    /// Get days since last accessed
    pub fn days_since_accessed(&self) -> Option<i64> {
        self.last_accessed.map(|last| {
            let now = chrono::Utc::now().naive_utc();
            (now - last).num_days()
        })
    }
}
