//! Integration tests for SyncRepository

#[cfg(test)]
mod tests {
    use crate::db::entities::sync_status::Model as SyncStatusModel;
    use crate::db::repository::Repository;
    use crate::db::repository::sync_repository::*;
    use anyhow::Result;
    use chrono::Utc;
    use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
    use std::sync::Arc;
    use tokio;

    async fn setup_test_repository() -> Result<(Arc<DatabaseConnection>, Arc<SyncRepositoryImpl>)> {
        use crate::db::connection::Database;
        use crate::db::entities::sources::ActiveModel as SourceActiveModel;
        use chrono::Utc;
        use sea_orm::{ActiveModelTrait, Set};
        use tempfile::TempDir;

        // Create temporary directory for test database
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test.db");

        // Need to leak the temp_dir to keep it alive for the test
        let _temp_dir = Box::leak(Box::new(temp_dir));

        // Create database connection
        let db = Database::connect(&db_path).await?;

        // Run migrations
        db.migrate().await?;

        let db_arc = db.get_connection();
        let repo = Arc::new(SyncRepositoryImpl::new(db_arc.clone()));

        // Create test source records that will be referenced by sync_status
        let test_sources = vec![
            "test-source-1",
            "test-source-2",
            "test-source-3",
            "test-source-4a",
            "test-source-4b",
            "test-source-4c",
            "test-source-4d",
            "test-source-5",
            "test-source-6",
            "test-source-7a",
            "test-source-7b",
            "test-source-8-0",
            "test-source-8-1",
            "test-source-8-2",
            "test-source-8-3",
            "test-source-8-4",
            "test-source-9",
            "test-source-10",
            "test-source-11",
            "test-source-12",
            "test-plex-1",
            "test-jellyfin-1",
            "test-local-1",
        ];

        for source_id in test_sources {
            let source = SourceActiveModel {
                id: Set(source_id.to_string()),
                name: Set(format!("Test Source {}", source_id)),
                source_type: Set("test".to_string()),
                auth_provider_id: Set(None),
                connection_url: Set(None),
                connections: Set(None),
                machine_id: Set(None),
                is_owned: Set(false),
                is_online: Set(true),
                last_sync: Set(None),
                last_connection_test: Set(None),
                connection_failure_count: Set(0),
                connection_quality: Set(None),
                auth_status: Set("authenticated".to_string()),
                last_auth_check: Set(None),
                created_at: Set(Utc::now().naive_utc()),
                updated_at: Set(Utc::now().naive_utc()),
            };
            source.insert(db_arc.as_ref()).await?;
        }

        // Clean up any existing test data
        use crate::db::entities::sync_status::Entity as SyncStatus;
        SyncStatus::delete_many()
            .filter(crate::db::entities::sync_status::Column::SourceId.contains("test-"))
            .exec(db_arc.as_ref())
            .await?;

        Ok((db_arc, repo))
    }

    #[tokio::test]
    async fn test_sync_status_crud_operations() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Test insert
        let sync = repo.start_sync("test-source-1", "full", Some(100)).await?;
        assert_eq!(sync.source_id, "test-source-1");
        assert_eq!(sync.sync_type, "full");
        assert_eq!(sync.status, "running");
        assert_eq!(sync.total_items, Some(100));
        assert!(sync.started_at.is_some());

        // Test find_by_id
        let found = repo.find_by_id(&sync.id.to_string()).await?;
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.id, sync.id);
        assert_eq!(found.source_id, sync.source_id);

        // Test update via complete_sync
        repo.complete_sync(sync.id, 95).await?;
        let updated = repo.find_by_id(&sync.id.to_string()).await?.unwrap();
        assert_eq!(updated.status, "completed");
        assert_eq!(updated.items_synced, 95);
        assert!(updated.completed_at.is_some());

        // Test delete
        repo.delete(&sync.id.to_string()).await?;
        let deleted = repo.find_by_id(&sync.id.to_string()).await?;
        assert!(deleted.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_find_by_source() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Create multiple syncs for same source
        let sync1 = repo.start_sync("test-source-2", "full", Some(50)).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        repo.complete_sync(sync1.id, 50).await?;

        let sync2 = repo
            .start_sync("test-source-2", "incremental", Some(20))
            .await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        repo.complete_sync(sync2.id, 20).await?;

        let sync3 = repo
            .start_sync("test-source-2", "library:lib1", None)
            .await?;

        // Test find_by_source (should return in reverse chronological order)
        let syncs = repo.find_by_source("test-source-2").await?;
        assert_eq!(syncs.len(), 3);
        assert_eq!(syncs[0].id, sync3.id); // Most recent
        assert_eq!(syncs[1].id, sync2.id);
        assert_eq!(syncs[2].id, sync1.id); // Oldest

        Ok(())
    }

    #[tokio::test]
    async fn test_find_latest_for_source() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Create multiple syncs with delays
        let sync1 = repo.start_sync("test-source-3", "full", Some(100)).await?;
        repo.complete_sync(sync1.id, 100).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let sync2 = repo
            .start_sync("test-source-3", "incremental", Some(30))
            .await?;
        repo.complete_sync(sync2.id, 30).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let sync3 = repo
            .start_sync("test-source-3", "incremental", Some(10))
            .await?;

        // Test find_latest_for_source
        let latest = repo.find_latest_for_source("test-source-3").await?;
        assert!(latest.is_some());
        let latest = latest.unwrap();
        assert_eq!(latest.id, sync3.id);
        assert_eq!(latest.status, "running");

        Ok(())
    }

    #[tokio::test]
    async fn test_find_running_syncs() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Create mix of running and completed syncs
        let sync1 = repo.start_sync("test-source-4a", "full", Some(100)).await?;
        repo.complete_sync(sync1.id, 100).await?;

        let _sync2 = repo
            .start_sync("test-source-4b", "incremental", Some(50))
            .await?;
        let _sync3 = repo
            .start_sync("test-source-4c", "library:lib2", None)
            .await?;

        let sync4 = repo
            .start_sync("test-source-4d", "media:movie1", Some(1))
            .await?;
        repo.fail_sync(sync4.id, "Network error").await?;

        // Test find_running
        let running = repo.find_running().await?;
        assert_eq!(running.len(), 2);
        assert!(running.iter().all(|s| s.status == "running"));

        Ok(())
    }

    #[tokio::test]
    async fn test_sync_lifecycle() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Test complete sync lifecycle
        let sync = repo.start_sync("test-source-5", "full", Some(200)).await?;
        assert_eq!(sync.status, "running");
        assert_eq!(sync.items_synced, 0);
        assert!(sync.started_at.is_some());
        assert!(sync.completed_at.is_none());
        assert!(sync.error_message.is_none());

        // Complete the sync
        repo.complete_sync(sync.id, 195).await?;
        let completed = repo.find_by_id(&sync.id.to_string()).await?.unwrap();
        assert_eq!(completed.status, "completed");
        assert_eq!(completed.items_synced, 195);
        assert!(completed.completed_at.is_some());
        assert!(completed.error_message.is_none());

        // Test failed sync lifecycle
        let fail_sync = repo
            .start_sync("test-source-5", "incremental", Some(50))
            .await?;
        repo.fail_sync(fail_sync.id, "Connection timeout").await?;

        let failed = repo.find_by_id(&fail_sync.id.to_string()).await?.unwrap();
        assert_eq!(failed.status, "failed");
        assert!(failed.completed_at.is_some());
        assert_eq!(failed.error_message, Some("Connection timeout".to_string()));

        Ok(())
    }

    // TODO: Re-enable once get_sync_stats is implemented
    // #[tokio::test]
    // async fn test_sync_stats() -> Result<()> {
    //     let (_db, repo) = setup_test_repository().await?;

    //     // Create various syncs for statistics
    //     let sync1 = repo.start_sync("test-source-6", "full", Some(100)).await?;
    //     tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    //     repo.complete_sync(sync1.id, 100).await?;

    //     let sync2 = repo
    //         .start_sync("test-source-6", "incremental", Some(50))
    //         .await?;
    //     tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    //     repo.complete_sync(sync2.id, 45).await?;

    //     let sync3 = repo
    //         .start_sync("test-source-6", "incremental", Some(30))
    //         .await?;
    //     repo.fail_sync(sync3.id, "API error").await?;

    //     let sync4 = repo
    //         .start_sync("test-source-6", "incremental", Some(25))
    //         .await?;
    //     tokio::time::sleep(tokio::time::Duration::from_millis(75)).await;
    //     repo.complete_sync(sync4.id, 25).await?;

    //     // Test get_sync_stats
    //     let stats = repo.get_sync_stats("test-source-6").await?;
    //     assert_eq!(stats.total_syncs, 4);
    //     assert_eq!(stats.successful_syncs, 3);
    //     assert_eq!(stats.failed_syncs, 1);
    //     assert_eq!(stats.total_items_synced, 170); // 100 + 45 + 25
    //     assert!(stats.last_sync_time.is_some());
    //     assert!(stats.average_sync_duration_secs.is_some());

    //     // Average duration should be reasonable
    //     let avg_duration = stats.average_sync_duration_secs.unwrap();
    //     assert!(avg_duration >= 0.0);
    //     assert!(avg_duration < 10.0); // Should be less than 10 seconds for test

    //     Ok(())
    // }

    #[tokio::test]
    async fn test_cleanup_old_records() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Create syncs for multiple sources
        for i in 0..5 {
            let sync = repo
                .start_sync("test-source-7a", &format!("sync-{}", i), Some(10))
                .await?;
            repo.complete_sync(sync.id, 10).await?;
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        for i in 0..3 {
            let sync = repo
                .start_sync("test-source-7b", &format!("sync-{}", i), Some(5))
                .await?;
            repo.complete_sync(sync.id, 5).await?;
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        // Keep only 2 most recent per source
        let deleted_count = repo.cleanup_old_records(2).await?;
        assert_eq!(deleted_count, 4); // 3 from source-7a + 1 from source-7b

        // Verify correct records remain
        let source_a_syncs = repo.find_by_source("test-source-7a").await?;
        assert_eq!(source_a_syncs.len(), 2);
        assert!(
            source_a_syncs
                .iter()
                .all(|s| s.sync_type == "sync-3" || s.sync_type == "sync-4")
        );

        let source_b_syncs = repo.find_by_source("test-source-7b").await?;
        assert_eq!(source_b_syncs.len(), 2);
        assert!(
            source_b_syncs
                .iter()
                .all(|s| s.sync_type == "sync-1" || s.sync_type == "sync-2")
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_sync_updates() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Start multiple syncs concurrently
        let mut handles = vec![];

        for i in 0..5 {
            let repo_clone = repo.clone();
            let handle = tokio::spawn(async move {
                repo_clone
                    .start_sync(&format!("test-source-8-{}", i), "concurrent", Some(100))
                    .await
            });
            handles.push(handle);
        }

        // Wait for all to complete
        let mut sync_ids = vec![];
        for handle in handles {
            let sync = handle.await??;
            sync_ids.push(sync.id);
        }

        // Complete them concurrently
        let mut handles = vec![];
        for (i, sync_id) in sync_ids.iter().enumerate() {
            let repo_clone = repo.clone();
            let sync_id = *sync_id;
            let handle = tokio::spawn(async move {
                repo_clone.complete_sync(sync_id, (i as i32 + 1) * 20).await
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await??;
        }

        // Verify all completed successfully
        for (i, sync_id) in sync_ids.iter().enumerate() {
            let sync = repo.find_by_id(&sync_id.to_string()).await?.unwrap();
            assert_eq!(sync.status, "completed");
            assert_eq!(sync.items_synced, (i as i32 + 1) * 20);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_sync_type_variations() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Test different sync types
        let full_sync = repo.start_sync("test-source-9", "full", Some(1000)).await?;
        assert_eq!(full_sync.sync_type, "full");

        let incremental_sync = repo
            .start_sync("test-source-9", "incremental", Some(50))
            .await?;
        assert_eq!(incremental_sync.sync_type, "incremental");

        let library_sync = repo
            .start_sync("test-source-9", "library:movies", None)
            .await?;
        assert_eq!(library_sync.sync_type, "library:movies");

        let media_sync = repo
            .start_sync("test-source-9", "media:movie-123", Some(1))
            .await?;
        assert_eq!(media_sync.sync_type, "media:movie-123");

        // Verify all are stored correctly
        let all_syncs = repo.find_by_source("test-source-9").await?;
        assert_eq!(all_syncs.len(), 4);

        Ok(())
    }

    #[tokio::test]
    async fn test_error_handling() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Test completing non-existent sync
        repo.complete_sync(99999, 100).await?; // Should not panic

        // Test failing non-existent sync
        repo.fail_sync(99999, "Error").await?; // Should not panic

        // Test finding by non-existent ID
        let not_found = repo.find_by_id("99999").await?;
        assert!(not_found.is_none());

        // Test finding by non-existent source
        let empty = repo.find_by_source("non-existent-source").await?;
        assert!(empty.is_empty());

        // TODO: Re-enable once get_sync_stats is implemented
        // // Test stats for non-existent source
        // let stats = repo.get_sync_stats("non-existent-source").await?;
        // assert_eq!(stats.total_syncs, 0);
        // assert_eq!(stats.successful_syncs, 0);
        // assert_eq!(stats.failed_syncs, 0);
        // assert_eq!(stats.total_items_synced, 0);
        // assert!(stats.last_sync_time.is_none());
        // assert!(stats.average_sync_duration_secs.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_sync_with_null_total_items() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Create sync without total_items
        let sync = repo.start_sync("test-source-10", "discovery", None).await?;
        assert!(sync.total_items.is_none());

        // Complete it
        repo.complete_sync(sync.id, 42).await?;
        let completed = repo.find_by_id(&sync.id.to_string()).await?.unwrap();
        assert_eq!(completed.items_synced, 42);
        assert!(completed.total_items.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_multiple_source_handling() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Create syncs for multiple distinct sources
        let sources = vec!["test-plex-1", "test-jellyfin-1", "test-local-1"];

        for source in &sources {
            for i in 0..3 {
                let sync = repo
                    .start_sync(source, &format!("sync-{}", i), Some(100))
                    .await?;
                repo.complete_sync(sync.id, 100).await?;
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        }

        // Verify each source has its own syncs
        for source in &sources {
            let syncs = repo.find_by_source(source).await?;
            assert_eq!(syncs.len(), 3);
            assert!(syncs.iter().all(|s| s.source_id == *source));
        }

        // Test latest for each source
        for source in &sources {
            let latest = repo.find_latest_for_source(source).await?;
            assert!(latest.is_some());
            assert_eq!(latest.unwrap().sync_type, "sync-2");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_large_error_messages() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        let sync = repo.start_sync("test-source-11", "full", Some(100)).await?;

        // Test with very long error message
        let long_error = "Error: ".to_string() + &"x".repeat(1000);
        repo.fail_sync(sync.id, &long_error).await?;

        let failed = repo.find_by_id(&sync.id.to_string()).await?.unwrap();
        assert_eq!(failed.error_message, Some(long_error));

        Ok(())
    }

    #[tokio::test]
    async fn test_repository_trait_implementation() -> Result<()> {
        let (_db, repo) = setup_test_repository().await?;

        // Test Repository trait methods
        let sync_model = SyncStatusModel {
            id: 0,
            source_id: "test-source-12".to_string(),
            sync_type: "manual".to_string(),
            status: "pending".to_string(),
            started_at: Some(Utc::now().naive_utc()),
            completed_at: None,
            items_synced: 0,
            total_items: Some(500),
            error_message: None,
        };

        // Insert via Repository trait
        let inserted = repo.insert(sync_model).await?;
        assert!(inserted.id > 0);

        // Update via Repository trait
        let mut updated_model = inserted.clone();
        updated_model.status = "running".to_string();
        updated_model.items_synced = 250;
        let updated = repo.update(updated_model).await?;
        assert_eq!(updated.status, "running");
        assert_eq!(updated.items_synced, 250);

        // Count via Repository trait
        let initial_count = repo.count().await?;
        assert!(initial_count > 0);

        // Find all via Repository trait
        let all = repo.find_all().await?;
        assert!(!all.is_empty());

        // Delete via Repository trait
        repo.delete(&inserted.id.to_string()).await?;
        let after_delete = repo.find_by_id(&inserted.id.to_string()).await?;
        assert!(after_delete.is_none());

        Ok(())
    }
}
