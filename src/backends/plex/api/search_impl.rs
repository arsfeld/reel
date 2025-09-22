use anyhow::Result;
use std::collections::HashMap;
use tracing::debug;

use super::client::PlexApi;
use super::search::PlexSearch;
use crate::models::MediaItem;

impl PlexApi {
    /// Perform a global search across all libraries
    pub async fn search_global(&self, query: &str, limit: Option<usize>) -> Result<Vec<MediaItem>> {
        let search_client = PlexSearch::new(
            self.base_url.clone(),
            self.auth_token.clone(),
            self.client.clone(),
        );

        search_client.global_search(query, limit).await
    }

    /// Search within a specific library
    pub async fn search_library(
        &self,
        library_id: &str,
        query: &str,
        media_type: Option<&str>,
        sort: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<MediaItem>> {
        let search_client = PlexSearch::new(
            self.base_url.clone(),
            self.auth_token.clone(),
            self.client.clone(),
        );

        search_client
            .library_search(library_id, query, media_type, sort, limit)
            .await
    }

    /// Perform an advanced search with custom parameters
    pub async fn search_advanced(
        &self,
        library_id: Option<&str>,
        params: HashMap<String, String>,
    ) -> Result<Vec<MediaItem>> {
        let search_client = PlexSearch::new(
            self.base_url.clone(),
            self.auth_token.clone(),
            self.client.clone(),
        );

        search_client.advanced_search(library_id, params).await
    }

    /// Search for content with specific filters
    pub async fn search_with_filters(
        &self,
        library_id: &str,
        query: Option<&str>,
        genre: Option<&str>,
        year: Option<u32>,
        rating_min: Option<f32>,
        unwatched: Option<bool>,
        sort: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<MediaItem>> {
        let mut params = HashMap::new();

        if let Some(q) = query {
            params.insert("title".to_string(), q.to_string());
        }

        if let Some(g) = genre {
            params.insert("genre".to_string(), g.to_string());
        }

        if let Some(y) = year {
            params.insert("year".to_string(), y.to_string());
        }

        if let Some(r) = rating_min {
            params.insert("audienceRating>>".to_string(), r.to_string());
        }

        if let Some(u) = unwatched {
            params.insert(
                "unwatched".to_string(),
                if u { "1".to_string() } else { "0".to_string() },
            );
        }

        if let Some(s) = sort {
            params.insert("sort".to_string(), s.to_string());
        }

        if let Some(l) = limit {
            params.insert("limit".to_string(), l.to_string());
        }

        self.search_advanced(Some(library_id), params).await
    }
}
