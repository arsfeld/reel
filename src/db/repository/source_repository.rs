use super::{BaseRepository, Repository};
use crate::db::entities::{Source, SourceActiveModel, SourceModel, sources};
use crate::events::{
    event_bus::EventBus,
    types::{DatabaseEvent, EventPayload, EventType},
};
use anyhow::Result;
use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    Set,
};
use std::sync::Arc;

/// Repository trait for Source entities
#[async_trait]

pub trait SourceRepository: Repository<SourceModel> {
    /// Find sources by type
    async fn find_by_type(&self, source_type: &str) -> Result<Vec<SourceModel>>;

    /// Find online sources
    async fn find_online(&self) -> Result<Vec<SourceModel>>;

    /// Update source online status
    async fn update_online_status(&self, id: &str, is_online: bool) -> Result<()>;

    /// Update last sync time
    async fn update_last_sync(&self, id: &str) -> Result<()>;

    /// Upsert a source (insert if not exists, update if exists)
    async fn upsert(&self, entity: SourceModel) -> Result<SourceModel>;

    /// Find sources by auth provider ID
    async fn find_by_auth_provider(&self, provider_id: &str) -> Result<Vec<SourceModel>>;

    /// Remove sources not in the given list for a provider (for cleanup)
    async fn cleanup_sources_for_provider(
        &self,
        provider_id: &str,
        keep_source_ids: &[String],
    ) -> Result<()>;

    /// Archive invalid sources that don't match config (instead of deleting)
    async fn archive_invalid_sources(
        &self,
        valid_source_ids: &[String],
    ) -> Result<Vec<SourceModel>>;

    /// Get all archived sources
    async fn find_archived(&self) -> Result<Vec<SourceModel>>;

    /// Clean up sources with unknown source_type (corrupted data)
    async fn cleanup_unknown_sources(&self) -> Result<Vec<SourceModel>>;
}

#[derive(Debug)]
pub struct SourceRepositoryImpl {
    base: BaseRepository,
}

impl SourceRepositoryImpl {
    pub fn new(db: Arc<DatabaseConnection>, event_bus: Arc<EventBus>) -> Self {
        Self {
            base: BaseRepository::new(db, event_bus),
        }
    }

    pub fn new_without_events(db: Arc<DatabaseConnection>) -> Self {
        Self {
            base: BaseRepository::new_without_events(db),
        }
    }
}

#[async_trait]
impl Repository<SourceModel> for SourceRepositoryImpl {
    type Entity = Source;

    async fn find_by_id(&self, id: &str) -> Result<Option<SourceModel>> {
        Ok(Source::find_by_id(id).one(self.base.db.as_ref()).await?)
    }

    async fn find_all(&self) -> Result<Vec<SourceModel>> {
        Ok(Source::find().all(self.base.db.as_ref()).await?)
    }

    async fn insert(&self, entity: SourceModel) -> Result<SourceModel> {
        let active_model = SourceActiveModel {
            id: Set(entity.id.clone()),
            name: Set(entity.name.clone()),
            source_type: Set(entity.source_type.clone()),
            auth_provider_id: Set(entity.auth_provider_id.clone()),
            connection_url: Set(entity.connection_url.clone()),
            is_online: Set(entity.is_online),
            last_sync: Set(entity.last_sync),
            created_at: Set(chrono::Utc::now().naive_utc()),
            updated_at: Set(chrono::Utc::now().naive_utc()),
        };

        let result = active_model.insert(self.base.db.as_ref()).await?;

        // Emit SourceAdded event
        let event = DatabaseEvent::new(
            EventType::SourceAdded,
            EventPayload::Source {
                id: result.id.clone(),
                source_type: result.source_type.clone(),
                is_online: Some(result.is_online),
            },
        );

        if let Some(event_bus) = &self.base.event_bus {
            if let Err(e) = event_bus.publish(event).await {
                tracing::warn!("Failed to publish SourceAdded event: {}", e);
            }
        }

        Ok(result)
    }

    async fn update(&self, entity: SourceModel) -> Result<SourceModel> {
        let mut active_model: SourceActiveModel = entity.clone().into();
        active_model.updated_at = Set(chrono::Utc::now().naive_utc());

        let result = active_model.update(self.base.db.as_ref()).await?;

        // Emit SourceUpdated event
        let event = DatabaseEvent::new(
            EventType::SourceUpdated,
            EventPayload::Source {
                id: result.id.clone(),
                source_type: result.source_type.clone(),
                is_online: Some(result.is_online),
            },
        );

        if let Some(event_bus) = &self.base.event_bus {
            if let Err(e) = event_bus.publish(event).await {
                tracing::warn!("Failed to publish SourceUpdated event: {}", e);
            }
        }

        Ok(result)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        // Get entity details before deleting
        let entity = self.find_by_id(id).await?;

        Source::delete_by_id(id).exec(self.base.db.as_ref()).await?;

        // Emit SourceRemoved event if entity existed
        if let Some(source) = entity {
            let event = DatabaseEvent::new(
                EventType::SourceRemoved,
                EventPayload::Source {
                    id: source.id.clone(),
                    source_type: source.source_type.clone(),
                    is_online: Some(source.is_online),
                },
            );

            if let Some(event_bus) = &self.base.event_bus {
                if let Err(e) = event_bus.publish(event).await {
                    tracing::warn!("Failed to publish SourceRemoved event: {}", e);
                }
            }
        }

        Ok(())
    }

    async fn count(&self) -> Result<u64> {
        Ok(Source::find().count(self.base.db.as_ref()).await?)
    }
}

#[async_trait]
impl SourceRepository for SourceRepositoryImpl {
    async fn find_by_type(&self, source_type: &str) -> Result<Vec<SourceModel>> {
        Ok(Source::find()
            .filter(sources::Column::SourceType.eq(source_type))
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn find_online(&self) -> Result<Vec<SourceModel>> {
        Ok(Source::find()
            .filter(sources::Column::IsOnline.eq(true))
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn update_online_status(&self, id: &str, is_online: bool) -> Result<()> {
        if let Some(source) = self.find_by_id(id).await? {
            let mut active_model: SourceActiveModel = source.clone().into();
            active_model.is_online = Set(is_online);
            active_model.updated_at = Set(chrono::Utc::now().naive_utc());
            active_model.update(self.base.db.as_ref()).await?;

            // Emit SourceOnlineStatusChanged event
            let event = DatabaseEvent::new(
                EventType::SourceOnlineStatusChanged,
                EventPayload::Source {
                    id: source.id.clone(),
                    source_type: source.source_type.clone(),
                    is_online: Some(is_online),
                },
            );

            if let Some(event_bus) = &self.base.event_bus {
                if let Err(e) = event_bus.publish(event).await {
                    tracing::warn!("Failed to publish SourceOnlineStatusChanged event: {}", e);
                }
            }
        }
        Ok(())
    }

    async fn update_last_sync(&self, id: &str) -> Result<()> {
        if let Some(source) = self.find_by_id(id).await? {
            let mut active_model: SourceActiveModel = source.into();
            active_model.last_sync = Set(Some(chrono::Utc::now().naive_utc()));
            active_model.updated_at = Set(chrono::Utc::now().naive_utc());
            active_model.update(self.base.db.as_ref()).await?;
        }
        Ok(())
    }

    async fn upsert(&self, entity: SourceModel) -> Result<SourceModel> {
        // Check if source exists
        if let Some(existing) = self.find_by_id(&entity.id).await? {
            // Update existing source
            let mut updated = entity.clone();
            updated.created_at = existing.created_at; // Keep original creation time
            self.update(updated).await
        } else {
            // Insert new source
            self.insert(entity).await
        }
    }

    async fn find_by_auth_provider(&self, provider_id: &str) -> Result<Vec<SourceModel>> {
        Ok(Source::find()
            .filter(sources::Column::AuthProviderId.eq(provider_id))
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn cleanup_sources_for_provider(
        &self,
        provider_id: &str,
        keep_source_ids: &[String],
    ) -> Result<()> {
        // Find all sources for this provider
        let existing_sources = self.find_by_auth_provider(provider_id).await?;

        // Delete sources that are not in the keep list
        for source in existing_sources {
            if !keep_source_ids.contains(&source.id) {
                tracing::info!(
                    "Cleaning up removed source: {} ({})",
                    source.name,
                    source.id
                );
                self.delete(&source.id).await?;
            }
        }

        Ok(())
    }

    async fn archive_invalid_sources(
        &self,
        valid_source_ids: &[String],
    ) -> Result<Vec<SourceModel>> {
        let all_sources = self.find_all().await?;
        let mut archived_sources = Vec::new();

        for source in all_sources {
            if !valid_source_ids.contains(&source.id) {
                // Don't archive sources that are just using old IDs but are still valid
                // Check if this source has a valid auth_provider_id and known source_type
                let should_archive = match (&source.auth_provider_id, &source.source_type) {
                    (Some(_provider_id), source_type)
                        if matches!(source_type.as_str(), "plex" | "jellyfin" | "local") =>
                    {
                        // This is a valid source type with an auth provider - don't archive it
                        // It might just be using an old ID format
                        tracing::info!(
                            "Keeping source with old ID but valid provider: {} ({}) - type: {}",
                            source.name,
                            source.id,
                            source.source_type
                        );
                        false
                    }
                    _ => {
                        // No auth provider or unknown source type - safe to archive
                        true
                    }
                };

                if should_archive {
                    tracing::warn!(
                        "Marking invalid source as offline: {} ({})",
                        source.name,
                        source.id
                    );
                    // Just mark as offline - keeps the data but removes it from active display
                    let mut archived_source = source.clone();
                    archived_source.is_online = false;
                    let updated = self.update(archived_source).await?;
                    archived_sources.push(updated);
                }
            }
        }

        Ok(archived_sources)
    }

    async fn find_archived(&self) -> Result<Vec<SourceModel>> {
        // Return offline sources (archived sources are just offline ones not in config)
        Ok(Source::find()
            .filter(sources::Column::IsOnline.eq(false))
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn cleanup_unknown_sources(&self) -> Result<Vec<SourceModel>> {
        // Find sources with unknown type - these are corrupted and should be removed
        let unknown_sources = Source::find()
            .filter(sources::Column::SourceType.eq("unknown"))
            .all(self.base.db.as_ref())
            .await?;

        let mut removed_sources = Vec::new();
        for source in unknown_sources {
            tracing::info!(
                "Removing corrupted source with unknown type: {} ({})",
                source.name,
                source.id
            );
            self.delete(&source.id).await?;
            removed_sources.push(source);
        }

        if !removed_sources.is_empty() {
            tracing::info!(
                "Cleaned up {} corrupted sources with unknown type",
                removed_sources.len()
            );
        }

        Ok(removed_sources)
    }
}
