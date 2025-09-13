// Test that our test infrastructure compiles and works
mod common;

#[cfg(test)]
mod tests {
    use super::common::{create_test_database, seed_test_data};

    #[tokio::test]
    async fn test_database_creation() {
        let db = create_test_database().await;
        assert!(db.connection().ping().await.is_ok());
    }

    #[tokio::test]
    async fn test_seed_data() {
        let db = create_test_database().await;
        let test_data = seed_test_data(&db).await;

        assert!(!test_data.source.id.is_empty());
        assert!(!test_data.library.id.is_empty());
        assert_eq!(test_data.media_items.len(), 5);
    }
}
