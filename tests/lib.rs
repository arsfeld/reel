// Test module declarations
pub mod common;

#[cfg(test)]
mod unit {
    pub mod services {
        // Include the sync service tests
        include!("unit/services/sync_service_test.rs");
    }
}

#[cfg(test)]
mod integration {
    // Include the message broker tests
    include!("integration/message_broker_test.rs");
}
