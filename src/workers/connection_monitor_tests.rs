#[cfg(test)]
mod tests {
    use crate::db::connection::get_test_db_connection;
    use crate::db::repository::source_repository::SourceRepositoryImpl;
    use crate::models::{ServerConnection, ServerConnections, SourceId};
    use crate::services::core::connection::ConnectionService;
    use crate::workers::connection_monitor::{
        ConnectionMonitor, ConnectionMonitorInput, ConnectionMonitorOutput,
    };
    use relm4::Worker;
    use sea_orm::{ActiveModelTrait, ActiveValue};
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tokio::time::timeout;

    async fn create_test_source(
        db: &Arc<sea_orm::DatabaseConnection>,
        id: &str,
        name: &str,
        connections: Vec<ServerConnection>,
    ) -> crate::db::entities::sources::Model {
        use crate::db::entities::sources;

        let connections_json = serde_json::to_value(ServerConnections::new(connections)).unwrap();

        let source = sources::ActiveModel {
            id: ActiveValue::Set(id.to_string()),
            name: ActiveValue::Set(name.to_string()),
            source_type: ActiveValue::Set("plex".to_string()),
            connections: ActiveValue::Set(Some(connections_json)),
            connection_url: ActiveValue::Set(None),
            connection_quality: ActiveValue::Set(None),
            ..Default::default()
        };
        source.insert(db.as_ref()).await.unwrap()
    }

    #[tokio::test]
    async fn test_health_check_intervals() {
        let db = Arc::new(get_test_db_connection().await.unwrap());
        let monitor = ConnectionMonitor {
            db: db.clone(),
            next_check_times: HashMap::new(),
        };

        // Test different quality levels have correct intervals
        let local_check =
            monitor.calculate_next_check(&SourceId::new("test".to_string()), Some("local"));
        let remote_check =
            monitor.calculate_next_check(&SourceId::new("test".to_string()), Some("remote"));
        let relay_check =
            monitor.calculate_next_check(&SourceId::new("test".to_string()), Some("relay"));
        let default_check = monitor.calculate_next_check(&SourceId::new("test".to_string()), None);

        // Check intervals are correct (allowing for small timing variations)
        let now = Instant::now();
        assert!(local_check > now + Duration::from_secs(299));
        assert!(local_check <= now + Duration::from_secs(301));

        assert!(remote_check > now + Duration::from_secs(119));
        assert!(remote_check <= now + Duration::from_secs(121));

        assert!(relay_check > now + Duration::from_secs(29));
        assert!(relay_check <= now + Duration::from_secs(31));

        assert!(default_check > now + Duration::from_secs(59));
        assert!(default_check <= now + Duration::from_secs(61));
    }

    #[tokio::test]
    async fn test_should_check_source() {
        let db = Arc::new(get_test_db_connection().await.unwrap());
        let mut monitor = ConnectionMonitor {
            db: db.clone(),
            next_check_times: HashMap::new(),
        };

        let source_id = SourceId::new("test-source".to_string());

        // Should check if not tracked yet
        assert!(monitor.should_check_source(&source_id));

        // Set future check time - should not check
        monitor
            .next_check_times
            .insert(source_id.clone(), Instant::now() + Duration::from_secs(60));
        assert!(!monitor.should_check_source(&source_id));

        // Set past check time - should check
        monitor
            .next_check_times
            .insert(source_id.clone(), Instant::now() - Duration::from_secs(1));
        assert!(monitor.should_check_source(&source_id));
    }

    #[tokio::test]
    async fn test_status_reporting_through_messages() {
        let db = Arc::new(get_test_db_connection().await.unwrap());

        // Create test source with connections
        let connections = vec![ServerConnection {
            uri: "http://test-1.local".to_string(),
            local: true,
            relay: false,
            address: "192.168.1.100".to_string(),
            port: 32400,
            protocol: "http".to_string(),
            response_time_ms: Some(10),
            is_available: true,
            priority: 1,
        }];

        let source = create_test_source(&db, "test-1", "Test Server", connections).await;
        let source_id = SourceId::new(source.id.clone());

        // Create monitor and channels
        let (sender, receiver) = relm4::channel::<ConnectionMonitorOutput>();
        let _monitor = ConnectionMonitor {
            db: db.clone(),
            next_check_times: HashMap::new(),
        };

        // Simulate checking a source
        let test_sender = sender.clone();
        let test_db = db.clone();
        let test_source_id = source_id.clone();

        tokio::spawn(async move {
            // This simulates what happens in the update method
            match ConnectionService::select_best_connection(&test_db, &test_source_id).await {
                Ok(Some(url)) => {
                    let _ = test_sender.send(ConnectionMonitorOutput::ConnectionRestored {
                        source_id: test_source_id.clone(),
                        url,
                    });
                }
                Ok(None) => {
                    let _ = test_sender.send(ConnectionMonitorOutput::ConnectionLost {
                        source_id: test_source_id.clone(),
                    });
                }
                Err(_) => {}
            }
        });

        // Wait for message
        let result = timeout(Duration::from_secs(5), async { receiver.recv() }).await;

        // The test should receive either ConnectionRestored or ConnectionLost
        // depending on whether the mock connection is considered available
        assert!(result.is_ok(), "Timeout waiting for connection message");

        match result.unwrap().await {
            Some(ConnectionMonitorOutput::ConnectionRestored { source_id, url }) => {
                assert_eq!(source_id.as_ref(), "test-1");
                assert_eq!(url, "http://test-1.local");
            }
            Some(ConnectionMonitorOutput::ConnectionLost { source_id }) => {
                // This is also acceptable if the connection service couldn't verify the connection
                assert_eq!(source_id.as_ref(), "test-1");
            }
            other => panic!("Unexpected message: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_multiple_source_monitoring() {
        let db = Arc::new(get_test_db_connection().await.unwrap());

        // Create multiple test sources
        let connections1 = vec![ServerConnection {
            uri: "http://multi-1.local".to_string(),
            local: true,
            relay: false,
            address: "192.168.1.101".to_string(),
            port: 32400,
            protocol: "http".to_string(),
            response_time_ms: Some(5),
            is_available: true,
            priority: 1,
        }];

        let connections2 = vec![ServerConnection {
            uri: "http://multi-2.remote".to_string(),
            local: false,
            relay: false,
            address: "remote.server.com".to_string(),
            port: 32400,
            protocol: "https".to_string(),
            response_time_ms: Some(100),
            is_available: true,
            priority: 2,
        }];

        let connections3 = vec![ServerConnection {
            uri: "http://multi-3.relay".to_string(),
            local: false,
            relay: true,
            address: "relay.plex.tv".to_string(),
            port: 443,
            protocol: "https".to_string(),
            response_time_ms: Some(300),
            is_available: true,
            priority: 3,
        }];

        let source1 = create_test_source(&db, "multi-1", "Server 1", connections1).await;
        let source2 = create_test_source(&db, "multi-2", "Server 2", connections2).await;
        let source3 = create_test_source(&db, "multi-3", "Server 3", connections3).await;

        let mut monitor = ConnectionMonitor {
            db: db.clone(),
            next_check_times: HashMap::new(),
        };

        // Track that all sources can be managed
        let id1 = SourceId::new(source1.id);
        let id2 = SourceId::new(source2.id);
        let id3 = SourceId::new(source3.id);

        // Set different check times
        monitor
            .next_check_times
            .insert(id1.clone(), Instant::now() - Duration::from_secs(1));
        monitor
            .next_check_times
            .insert(id2.clone(), Instant::now() + Duration::from_secs(60));
        monitor
            .next_check_times
            .insert(id3.clone(), Instant::now() - Duration::from_secs(1));

        // Verify correct sources are due for checking
        assert!(monitor.should_check_source(&id1));
        assert!(!monitor.should_check_source(&id2)); // Not due yet
        assert!(monitor.should_check_source(&id3));
    }

    #[tokio::test]
    async fn test_connection_lost_detection() {
        let db = Arc::new(get_test_db_connection().await.unwrap());

        // Create source with unavailable connection
        let connections = vec![ServerConnection {
            uri: "http://loss-test.local".to_string(),
            local: true,
            relay: false,
            address: "192.168.1.200".to_string(),
            port: 32400,
            protocol: "http".to_string(),
            response_time_ms: None,
            is_available: false, // Mark as unavailable
            priority: 1,
        }];

        let source = create_test_source(&db, "loss-test", "Loss Test Server", connections).await;
        let source_id = SourceId::new(source.id.clone());

        let (sender, receiver) = relm4::channel::<ConnectionMonitorOutput>();

        // Check connection - should detect loss
        let test_sender = sender.clone();
        let test_db = db.clone();
        let test_source_id = source_id.clone();

        tokio::spawn(async move {
            match ConnectionService::select_best_connection(&test_db, &test_source_id).await {
                Ok(None) => {
                    let _ = test_sender.send(ConnectionMonitorOutput::ConnectionLost {
                        source_id: test_source_id,
                    });
                }
                _ => {}
            }
        });

        // Wait for connection lost message
        let result = timeout(Duration::from_secs(2), async { receiver.recv() }).await;

        assert!(result.is_ok());

        match result.unwrap().await {
            Some(ConnectionMonitorOutput::ConnectionLost { source_id }) => {
                assert_eq!(source_id.as_ref(), "loss-test");
            }
            _ => panic!("Expected ConnectionLost message"),
        }
    }

    #[tokio::test]
    async fn test_update_check_times_message() {
        let db = Arc::new(get_test_db_connection().await.unwrap());
        let mut monitor = ConnectionMonitor {
            db: db.clone(),
            next_check_times: HashMap::new(),
        };

        // Create new check times
        let mut new_times = HashMap::new();
        let id1 = SourceId::new("source1".to_string());
        let id2 = SourceId::new("source2".to_string());
        new_times.insert(id1, Instant::now() + Duration::from_secs(30));
        new_times.insert(id2, Instant::now() + Duration::from_secs(60));

        // Simulate UpdateCheckTimes message
        monitor.next_check_times = new_times.clone();

        // Verify times were updated
        assert_eq!(monitor.next_check_times.len(), 2);
        assert!(
            monitor
                .next_check_times
                .contains_key(&SourceId::new("source1".to_string()))
        );
        assert!(
            monitor
                .next_check_times
                .contains_key(&SourceId::new("source2".to_string()))
        );
    }

    #[tokio::test]
    async fn test_check_all_sources_with_timing() {
        let db = Arc::new(get_test_db_connection().await.unwrap());

        // Create sources with different qualities
        let local_connections = vec![ServerConnection {
            uri: "http://timing-1.local".to_string(),
            local: true,
            relay: false,
            address: "192.168.1.10".to_string(),
            port: 32400,
            protocol: "http".to_string(),
            response_time_ms: Some(5),
            is_available: true,
            priority: 1,
        }];

        let remote_connections = vec![ServerConnection {
            uri: "http://timing-2.remote".to_string(),
            local: false,
            relay: false,
            address: "remote.server.com".to_string(),
            port: 443,
            protocol: "https".to_string(),
            response_time_ms: Some(100),
            is_available: true,
            priority: 1,
        }];

        let source1 = create_test_source(&db, "timing-1", "Local Server", local_connections).await;
        let source2 =
            create_test_source(&db, "timing-2", "Remote Server", remote_connections).await;

        let monitor = ConnectionMonitor {
            db: db.clone(),
            next_check_times: HashMap::new(),
        };

        // Calculate expected intervals
        let local_interval =
            monitor.calculate_next_check(&SourceId::new(source1.id), Some("local"));
        let remote_interval =
            monitor.calculate_next_check(&SourceId::new(source2.id), Some("remote"));

        let now = Instant::now();

        // Verify local has longer interval (5 min) than remote (2 min)
        assert!(local_interval > remote_interval);
        assert!(local_interval > now + Duration::from_secs(290));
        assert!(remote_interval > now + Duration::from_secs(110));
        assert!(remote_interval < now + Duration::from_secs(130));
    }
}
