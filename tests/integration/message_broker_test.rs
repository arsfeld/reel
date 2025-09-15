#[cfg(test)]
mod message_broker_tests {
    use reel::platforms::relm4::components::shared::broker::{
        BROKER, BrokerMessage, SourceMessage, DataMessage, NavigationMessage, PlaybackMessage
    };
    use relm4::Sender;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn test_message_broker_routing() {
        // Create test receivers
        let (tx1, mut rx1) = relm4::channel::<BrokerMessage>();
        let (tx2, mut rx2) = relm4::channel::<BrokerMessage>();

        // Subscribe components
        BROKER.subscribe("TestComponent1".to_string(), tx1).await;
        BROKER.subscribe("TestComponent2".to_string(), tx2).await;

        // Send a message
        let test_message = BrokerMessage::Source(SourceMessage::SyncStarted {
            source_id: "test-source".to_string(),
            total_items: Some(100),
        });

        BROKER.broadcast(test_message.clone()).await;

        // Both subscribers should receive the message
        tokio::time::timeout(std::time::Duration::from_millis(100), async {
            let msg1 = rx1.recv().await.unwrap();
            let msg2 = rx2.recv().await.unwrap();

            assert!(matches!(msg1, BrokerMessage::Source(SourceMessage::SyncStarted { .. })));
            assert!(matches!(msg2, BrokerMessage::Source(SourceMessage::SyncStarted { .. })));
        })
        .await
        .expect("Should receive messages within timeout");

        // Cleanup
        BROKER.unsubscribe("TestComponent1").await;
        BROKER.unsubscribe("TestComponent2").await;
    }

    #[tokio::test]
    async fn test_message_broker_subscription() {
        let (tx, mut rx) = relm4::channel::<BrokerMessage>();
        let component_id = "TestSubscriber".to_string();

        // Subscribe
        BROKER.subscribe(component_id.clone(), tx).await;

        // Verify subscription by sending a message
        BROKER.notify_sync_started("test-source".to_string(), Some(50)).await;

        let msg = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            rx.recv()
        ).await.expect("Should receive message").unwrap();

        assert!(matches!(
            msg,
            BrokerMessage::Source(SourceMessage::SyncStarted { source_id, total_items: Some(50) })
            if source_id == "test-source"
        ));

        // Cleanup
        BROKER.unsubscribe(&component_id).await;
    }

    #[tokio::test]
    async fn test_message_broker_unsubscription() {
        let (tx, mut rx) = relm4::channel::<BrokerMessage>();
        let component_id = "TestUnsubscriber".to_string();

        // Subscribe
        BROKER.subscribe(component_id.clone(), tx).await;

        // Unsubscribe
        BROKER.unsubscribe(&component_id).await;

        // Send a message
        BROKER.notify_sync_started("test-source".to_string(), None).await;

        // Should not receive message after unsubscription
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            rx.recv()
        ).await;

        assert!(result.is_err(), "Should not receive messages after unsubscription");
    }

    #[tokio::test]
    async fn test_message_broker_error_handling() {
        // Test that broker handles dropped receivers gracefully
        let component_id = "TestDroppedReceiver".to_string();

        {
            let (tx, _rx) = relm4::channel::<BrokerMessage>();
            BROKER.subscribe(component_id.clone(), tx).await;
            // rx is dropped here
        }

        // Sending should not panic even with dropped receiver
        BROKER.notify_sync_error(
            "test-source".to_string(),
            "Test error".to_string()
        ).await;

        // Verify subscriber count is correct
        let count = BROKER.subscriber_count().await;
        assert_eq!(count, 1, "Should still have the dropped subscriber entry");

        // Cleanup
        BROKER.unsubscribe(&component_id).await;
    }

    #[tokio::test]
    async fn test_message_broker_performance() {
        // Test broker performance with many subscribers and messages
        let mut subscribers = vec![];
        let num_subscribers = 100;
        let num_messages = 1000;

        // Create many subscribers
        for i in 0..num_subscribers {
            let (tx, rx) = relm4::channel::<BrokerMessage>();
            let component_id = format!("PerfTestComponent{}", i);
            BROKER.subscribe(component_id.clone(), tx).await;
            subscribers.push((component_id, rx));
        }

        let start = std::time::Instant::now();

        // Send many messages
        for i in 0..num_messages {
            BROKER.notify_sync_progress(
                "test-source".to_string(),
                i,
                num_messages
            ).await;
        }

        let duration = start.elapsed();
        println!("Sent {} messages to {} subscribers in {:?}",
                 num_messages, num_subscribers, duration);

        // Verify performance is acceptable (should be < 1 second for this workload)
        assert!(duration.as_secs() < 1, "Broker performance is too slow");

        // Cleanup
        for (component_id, _) in subscribers {
            BROKER.unsubscribe(&component_id).await;
        }
    }

    #[tokio::test]
    async fn test_sync_notification_helpers() {
        let (tx, mut rx) = relm4::channel::<BrokerMessage>();
        BROKER.subscribe("TestHelpers".to_string(), tx).await;

        // Test notify_sync_started
        BROKER.notify_sync_started("source1".to_string(), Some(100)).await;
        let msg = rx.recv().await.unwrap();
        assert!(matches!(
            msg,
            BrokerMessage::Source(SourceMessage::SyncStarted { source_id, total_items: Some(100) })
            if source_id == "source1"
        ));

        // Test notify_sync_progress (sends two messages)
        BROKER.notify_sync_progress("source1".to_string(), 50, 100).await;

        // Should receive SourceMessage
        let msg = rx.recv().await.unwrap();
        assert!(matches!(
            msg,
            BrokerMessage::Source(SourceMessage::SyncProgress { source_id, current: 50, total: 100 })
            if source_id == "source1"
        ));

        // Should also receive DataMessage
        let msg = rx.recv().await.unwrap();
        assert!(matches!(
            msg,
            BrokerMessage::Data(DataMessage::SyncProgress { source_id, current: 50, total: 100 })
            if source_id == "source1"
        ));

        // Test notify_sync_completed (sends two messages)
        BROKER.notify_sync_completed("source1".to_string(), 100).await;

        let msg = rx.recv().await.unwrap();
        assert!(matches!(
            msg,
            BrokerMessage::Source(SourceMessage::SyncCompleted { source_id, items_synced: 100 })
            if source_id == "source1"
        ));

        let msg = rx.recv().await.unwrap();
        assert!(matches!(
            msg,
            BrokerMessage::Data(DataMessage::LoadComplete { source })
            if source == "source1"
        ));

        // Test notify_sync_error (sends two messages)
        BROKER.notify_sync_error("source1".to_string(), "Test error".to_string()).await;

        let msg = rx.recv().await.unwrap();
        assert!(matches!(
            msg,
            BrokerMessage::Source(SourceMessage::SyncError { source_id, error })
            if source_id == "source1" && error == "Test error"
        ));

        let msg = rx.recv().await.unwrap();
        assert!(matches!(
            msg,
            BrokerMessage::Data(DataMessage::LoadError { source, error })
            if source == "source1" && error == "Test error"
        ));

        // Cleanup
        BROKER.unsubscribe("TestHelpers").await;
    }

    #[tokio::test]
    async fn test_data_notification_helpers() {
        let (tx, mut rx) = relm4::channel::<BrokerMessage>();
        BROKER.subscribe("TestDataHelpers".to_string(), tx).await;

        // Test notify_loading_started
        BROKER.notify_loading_started("source1".to_string()).await;
        let msg = rx.recv().await.unwrap();
        assert!(matches!(
            msg,
            BrokerMessage::Data(DataMessage::Loading { source })
            if source == "source1"
        ));

        // Test notify_media_updated
        BROKER.notify_media_updated("media123".to_string()).await;
        let msg = rx.recv().await.unwrap();
        assert!(matches!(
            msg,
            BrokerMessage::Data(DataMessage::MediaUpdated { media_id })
            if media_id == "media123"
        ));

        // Test notify_library_updated
        BROKER.notify_library_updated("lib456".to_string()).await;
        let msg = rx.recv().await.unwrap();
        assert!(matches!(
            msg,
            BrokerMessage::Data(DataMessage::LibraryUpdated { library_id })
            if library_id == "lib456"
        ));

        // Cleanup
        BROKER.unsubscribe("TestDataHelpers").await;
    }

    #[tokio::test]
    async fn test_send_to_specific_component() {
        let (tx1, mut rx1) = relm4::channel::<BrokerMessage>();
        let (tx2, mut rx2) = relm4::channel::<BrokerMessage>();

        BROKER.subscribe("Component1".to_string(), tx1).await;
        BROKER.subscribe("Component2".to_string(), tx2).await;

        // Send to specific component
        let message = BrokerMessage::Navigation(NavigationMessage::ToHome);
        BROKER.send_to("Component1", message.clone()).await;

        // Only Component1 should receive
        let result1 = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            rx1.recv()
        ).await;
        assert!(result1.is_ok(), "Component1 should receive the message");

        let result2 = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            rx2.recv()
        ).await;
        assert!(result2.is_err(), "Component2 should not receive the message");

        // Cleanup
        BROKER.unsubscribe("Component1").await;
        BROKER.unsubscribe("Component2").await;
    }
}