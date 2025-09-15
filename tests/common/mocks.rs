use async_trait::async_trait;
use reel::{
    backends::traits::MediaBackend, models::*, player::traits::MediaPlayer, utils::error::RResult,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct MockBackend {
    pub name: String,
    pub libraries: Vec<Library>,
    pub media_items: HashMap<String, Vec<Movie>>,
    pub shows: HashMap<String, Vec<Show>>,
    pub error_mode: Arc<Mutex<Option<String>>>,
}

impl MockBackend {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            libraries: vec![
                Library {
                    id: "lib1".to_string(),
                    source_id: "mock".to_string(),
                    name: "Mock Movies".to_string(),
                    library_type: MediaType::Movie,
                    item_count: 5,
                    ..Default::default()
                },
                Library {
                    id: "lib2".to_string(),
                    source_id: "mock".to_string(),
                    name: "Mock Shows".to_string(),
                    library_type: MediaType::Show,
                    item_count: 3,
                    ..Default::default()
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

    pub fn inject_error(&self, error: String) {
        *self.error_mode.lock().unwrap() = Some(error);
    }

    pub fn clear_error(&self) {
        *self.error_mode.lock().unwrap() = None;
    }

    fn check_error(&self) -> RResult<()> {
        if let Some(error) = self.error_mode.lock().unwrap().clone() {
            return Err(reel::utils::error::AppError::BackendError(error));
        }
        Ok(())
    }
}

#[async_trait]
impl MediaBackend for MockBackend {
    fn name(&self) -> &str {
        &self.name
    }

    fn backend_type(&self) -> ServerType {
        ServerType::Plex
    }

    async fn authenticate(&self, _credentials: Credentials) -> RResult<User> {
        self.check_error()?;
        Ok(User {
            id: "mock_user".to_string(),
            username: "test_user".to_string(),
            email: Some("test@example.com".to_string()),
            ..Default::default()
        })
    }

    async fn get_libraries(&self) -> RResult<Vec<Library>> {
        self.check_error()?;
        Ok(self.libraries.clone())
    }

    async fn get_movies(&self, library_id: &str) -> RResult<Vec<Movie>> {
        self.check_error()?;
        Ok(self
            .media_items
            .get(library_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn get_shows(&self, library_id: &str) -> RResult<Vec<Show>> {
        self.check_error()?;
        Ok(self.shows.get(library_id).cloned().unwrap_or_default())
    }

    async fn get_episodes(&self, _show_id: &str) -> RResult<Vec<Episode>> {
        self.check_error()?;
        Ok(vec![])
    }

    async fn get_movie_details(&self, movie_id: &str) -> RResult<Movie> {
        self.check_error()?;
        for movies in self.media_items.values() {
            if let Some(movie) = movies.iter().find(|m| m.id == movie_id) {
                return Ok(movie.clone());
            }
        }
        Err(reel::utils::error::AppError::NotFound)
    }

    async fn get_show_details(&self, show_id: &str) -> RResult<Show> {
        self.check_error()?;
        for shows in self.shows.values() {
            if let Some(show) = shows.iter().find(|s| s.id == show_id) {
                return Ok(show.clone());
            }
        }
        Err(reel::utils::error::AppError::NotFound)
    }

    async fn get_stream_url(&self, _item_id: &str) -> RResult<String> {
        self.check_error()?;
        Ok("http://localhost/stream/test.mp4".to_string())
    }

    async fn mark_watched(&self, _item_id: &str) -> RResult<()> {
        self.check_error()?;
        Ok(())
    }

    async fn mark_unwatched(&self, _item_id: &str) -> RResult<()> {
        self.check_error()?;
        Ok(())
    }

    async fn update_progress(&self, _item_id: &str, _position_ms: i64) -> RResult<()> {
        self.check_error()?;
        Ok(())
    }

    async fn search(&self, query: &str) -> RResult<Vec<MediaItem>> {
        self.check_error()?;
        let mut results = Vec::new();

        for movies in self.media_items.values() {
            for movie in movies {
                if movie.title.to_lowercase().contains(&query.to_lowercase()) {
                    results.push(MediaItem::Movie(movie.clone()));
                }
            }
        }

        Ok(results)
    }

    async fn get_continue_watching(&self) -> RResult<Vec<MediaItem>> {
        self.check_error()?;
        Ok(vec![])
    }

    async fn get_recently_added(&self, _limit: usize) -> RResult<Vec<MediaItem>> {
        self.check_error()?;
        Ok(vec![])
    }

    async fn health_check(&self) -> RResult<bool> {
        self.check_error()?;
        Ok(true)
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

    fn check_error(&self) -> RResult<()> {
        if let Some(error) = self.error_mode.lock().unwrap().clone() {
            return Err(reel::utils::error::AppError::PlayerError(error));
        }
        Ok(())
    }
}

impl MediaPlayer for MockPlayer {
    fn load(&mut self, _url: &str) -> RResult<()> {
        self.check_error()?;
        *self.state.lock().unwrap() = PlayerState::Playing;
        *self.duration_ms.lock().unwrap() = 120 * 60 * 1000; // 2 hours
        Ok(())
    }

    fn play(&mut self) -> RResult<()> {
        self.check_error()?;
        *self.state.lock().unwrap() = PlayerState::Playing;
        Ok(())
    }

    fn pause(&mut self) -> RResult<()> {
        self.check_error()?;
        *self.state.lock().unwrap() = PlayerState::Paused;
        Ok(())
    }

    fn stop(&mut self) -> RResult<()> {
        self.check_error()?;
        *self.state.lock().unwrap() = PlayerState::Idle;
        *self.position_ms.lock().unwrap() = 0;
        Ok(())
    }

    fn seek(&mut self, position_ms: i64) -> RResult<()> {
        self.check_error()?;
        *self.position_ms.lock().unwrap() = position_ms;
        Ok(())
    }

    fn get_position(&self) -> i64 {
        *self.position_ms.lock().unwrap()
    }

    fn get_duration(&self) -> i64 {
        *self.duration_ms.lock().unwrap()
    }

    fn set_volume(&mut self, volume: f64) -> RResult<()> {
        self.check_error()?;
        *self.volume.lock().unwrap() = volume.clamp(0.0, 1.0);
        Ok(())
    }

    fn get_volume(&self) -> f64 {
        *self.volume.lock().unwrap()
    }

    fn get_state(&self) -> PlayerState {
        *self.state.lock().unwrap()
    }

    fn set_subtitle_track(&mut self, _track_id: Option<i32>) -> RResult<()> {
        self.check_error()?;
        Ok(())
    }

    fn set_audio_track(&mut self, _track_id: i32) -> RResult<()> {
        self.check_error()?;
        Ok(())
    }

    fn get_subtitle_tracks(&self) -> Vec<String> {
        vec!["English".to_string(), "Spanish".to_string()]
    }

    fn get_audio_tracks(&self) -> Vec<String> {
        vec!["English 5.1".to_string(), "Spanish 2.0".to_string()]
    }
}

pub struct MockKeyring {
    storage: Arc<Mutex<HashMap<String, String>>>,
}

impl MockKeyring {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn set(&self, service: &str, username: &str, password: &str) -> RResult<()> {
        let key = format!("{}-{}", service, username);
        self.storage
            .lock()
            .unwrap()
            .insert(key, password.to_string());
        Ok(())
    }

    pub fn get(&self, service: &str, username: &str) -> RResult<String> {
        let key = format!("{}-{}", service, username);
        self.storage
            .lock()
            .unwrap()
            .get(&key)
            .cloned()
            .ok_or(gnome_reel::utils::error::AppError::NotFound)
    }

    pub fn delete(&self, service: &str, username: &str) -> RResult<()> {
        let key = format!("{}-{}", service, username);
        self.storage.lock().unwrap().remove(&key);
        Ok(())
    }
}
