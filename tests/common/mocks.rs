use anyhow::Result;
use async_trait::async_trait;
use reel::{
    backends::traits::{MediaBackend, SearchResults, WatchStatus},
    models::*,
    player::PlayerState,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct MockBackend {
    pub name: String,
    pub libraries: Vec<Library>,
    pub media_items: HashMap<String, Vec<Movie>>,
    pub shows: HashMap<String, Vec<Show>>,
    pub error_mode: Arc<Mutex<Option<String>>>,
}

impl std::fmt::Debug for MockBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockBackend")
            .field("name", &self.name)
            .field("libraries", &self.libraries.len())
            .finish()
    }
}

impl MockBackend {
    pub fn new() -> Self {
        Self {
            name: "MockBackend".to_string(),
            libraries: vec![
                Library {
                    id: "lib1".to_string(),
                    title: "Mock Movies".to_string(),
                    library_type: LibraryType::Movies,
                    icon: None,
                    item_count: Some(5),
                },
                Library {
                    id: "lib2".to_string(),
                    title: "Mock Shows".to_string(),
                    library_type: LibraryType::Shows,
                    icon: None,
                    item_count: Some(3),
                },
            ],
            media_items: HashMap::new(),
            shows: HashMap::new(),
            error_mode: Arc::new(Mutex::new(None)),
        }
    }

    pub fn with_movies(mut self, library_id: &str, movies: Vec<Movie>) -> Self {
        self.media_items.insert(library_id.to_string(), movies);
        self
    }

    pub fn with_shows(mut self, library_id: &str, shows: Vec<Show>) -> Self {
        self.shows.insert(library_id.to_string(), shows);
        self
    }

    pub fn add_library(&mut self, library: Library) {
        self.libraries.push(library);
    }

    pub fn inject_error(&self, error: String) {
        *self.error_mode.lock().unwrap() = Some(error);
    }

    pub fn clear_error(&self) {
        *self.error_mode.lock().unwrap() = None;
    }

    fn check_error(&self) -> Result<()> {
        if let Some(error) = self.error_mode.lock().unwrap().clone() {
            return Err(anyhow::anyhow!("Backend error: {}", error));
        }
        Ok(())
    }
}

#[async_trait]
impl MediaBackend for MockBackend {
    async fn initialize(&self) -> Result<Option<User>> {
        self.check_error()?;
        Ok(Some(User {
            id: "mock_user".to_string(),
            username: "test_user".to_string(),
            email: Some("test@example.com".to_string()),
            avatar_url: None,
        }))
    }

    async fn is_initialized(&self) -> bool {
        self.error_mode.lock().unwrap().is_none()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn authenticate(&self, _credentials: Credentials) -> Result<User> {
        self.check_error()?;
        Ok(User {
            id: "mock_user".to_string(),
            username: "test_user".to_string(),
            email: Some("test@example.com".to_string()),
            avatar_url: None,
        })
    }

    async fn get_libraries(&self) -> Result<Vec<Library>> {
        self.check_error()?;
        Ok(self.libraries.clone())
    }

    async fn get_movies(&self, library_id: &LibraryId) -> Result<Vec<Movie>> {
        self.check_error()?;
        Ok(self
            .media_items
            .get(library_id.as_str())
            .cloned()
            .unwrap_or_default())
    }

    async fn get_shows(&self, library_id: &LibraryId) -> Result<Vec<Show>> {
        self.check_error()?;
        Ok(self
            .shows
            .get(library_id.as_str())
            .cloned()
            .unwrap_or_default())
    }

    async fn get_episodes(&self, _show_id: &ShowId, _season: u32) -> Result<Vec<Episode>> {
        self.check_error()?;
        Ok(vec![])
    }

    async fn get_stream_url(&self, _media_id: &MediaItemId) -> Result<StreamInfo> {
        self.check_error()?;
        Ok(StreamInfo {
            url: "http://localhost/stream/test.mp4".to_string(),
            subtitles: vec![],
            audio_tracks: vec![],
            chapters: vec![],
            duration: Some(Duration::from_secs(120 * 60)),
        })
    }

    async fn update_progress(
        &self,
        _media_id: &MediaItemId,
        _position: Duration,
        _duration: Duration,
    ) -> Result<()> {
        self.check_error()?;
        Ok(())
    }

    async fn mark_watched(&self, _media_id: &MediaItemId) -> Result<()> {
        self.check_error()?;
        Ok(())
    }

    async fn mark_unwatched(&self, _media_id: &MediaItemId) -> Result<()> {
        self.check_error()?;
        Ok(())
    }

    async fn get_watch_status(&self, _media_id: &MediaItemId) -> Result<WatchStatus> {
        self.check_error()?;
        Ok(WatchStatus {
            watched: false,
            position: None,
            last_watched: None,
        })
    }

    async fn search(&self, query: &str) -> Result<SearchResults> {
        self.check_error()?;
        let mut items = Vec::new();

        for movies in self.media_items.values() {
            for movie in movies {
                if movie.title.to_lowercase().contains(&query.to_lowercase()) {
                    items.push(MediaItem::Movie(movie.clone()));
                }
            }
        }

        Ok(SearchResults {
            movies: vec![],
            shows: vec![],
            episodes: vec![],
            albums: vec![],
            tracks: vec![],
            photos: vec![],
            items,
        })
    }
}

pub struct MockPlayer {
    pub state: Arc<Mutex<PlayerState>>,
    pub position_ms: Arc<Mutex<i64>>,
    pub duration_ms: Arc<Mutex<i64>>,
    pub volume: Arc<Mutex<f64>>,
    pub error_mode: Arc<Mutex<Option<String>>>,
}

impl MockPlayer {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(PlayerState::Idle)),
            position_ms: Arc::new(Mutex::new(0)),
            duration_ms: Arc::new(Mutex::new(0)),
            volume: Arc::new(Mutex::new(1.0)),
            error_mode: Arc::new(Mutex::new(None)),
        }
    }

    pub fn inject_error(&self, error: String) {
        *self.error_mode.lock().unwrap() = Some(error);
    }

    pub fn clear_error(&self) {
        *self.error_mode.lock().unwrap() = None;
    }
}
