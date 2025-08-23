# Reel - Technical Architecture Document

## Overview

Reel is built using Rust with GTK4/libadwaita for the UI, GStreamer for media playback, and an abstracted backend system supporting multiple media servers. The architecture emphasizes performance, maintainability, and extensibility.

## Technology Stack

### Core Technologies
- **Language**: Rust (2021 edition)
- **UI Framework**: GTK4 + libadwaita
- **Media Playback**: GStreamer
- **Async Runtime**: Tokio
- **HTTP Client**: Reqwest
- **Serialization**: Serde
- **Database**: SQLite (via sqlx) for caching
- **State Management**: Custom reactive system

### Development Tools
- **Build System**: Cargo + Meson (for GNOME integration)
- **Testing**: Built-in Rust testing + mockito
- **Documentation**: rustdoc
- **CI/CD**: GitHub Actions
- **Packaging**: Flatpak (primary), AUR, deb/rpm

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        UI Layer (GTK4)                      │
├─────────────────────────────────────────────────────────────┤
│                    Application State                         │
├─────────────────────────────────────────────────────────────┤
│                     Service Layer                           │
├─────────────────────────────────────────────────────────────┤
│                  Source Management Layer                     │
├─────────────────────────────────────────────────────────────┤
│                  Sync & Cache Manager                       │
├─────────────────────────────────────────────────────────────┤
│                 Metadata Provider Layer                     │
├─────────────────────────────────────────────────────────────┤
│                 Account Management Layer                     │
├─────────────────────────────────────────────────────────────┤
│                    Backend Abstraction                      │
├──────────────┬──────────────┬──────────────┬───────────────┤
│    Plex      │   Jellyfin   │    Local     │   Network     │
└──────────────┴──────────────┴──────────────┴───────────────┘
```

### Conceptual Model

```
User
 ├── Plex Account 1 ─────┬── Source: Home Server
 │                        │    ├── Library: Movies
 │                        │    ├── Library: TV Shows
 │                        │    └── Library: Kids Movies
 │                        └── Source: Friend's Server
 │                             └── Library: Shared Movies
 ├── Plex Account 2 ──────── Source: Family Server
 │                             ├── Library: Family Videos
 │                             └── Library: Photos
 ├── Jellyfin Credentials ─── Source: Jellyfin Server
 │                             ├── Library: Anime
 │                             └── Library: Documentaries
 └── Local ───────────────── Source: Local Files
                               ├── Library: /home/user/Videos
                               ├── Library: /media/nas/movies
                               └── Library: /media/usb/tvshows
```

## Module Structure

```
reel/
├── src/
│   ├── main.rs                 # Application entry point
│   ├── app.rs                  # Main application struct
│   ├── config.rs               # Configuration management
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── window.rs           # Main application window
│   │   ├── widgets/
│   │   │   ├── player.rs       # Video player widget
│   │   │   ├── library.rs      # Library browser
│   │   │   ├── details.rs      # Media details view
│   │   │   └── ...
│   │   ├── pages/
│   │   │   ├── movies.rs
│   │   │   ├── shows.rs
│   │   │   ├── settings.rs
│   │   │   └── ...
│   │   └── components/
│   │       ├── media_card.rs
│   │       ├── episode_row.rs
│   │       └── ...
│   ├── services/
│   │   ├── mod.rs
│   │   ├── auth.rs             # Authentication service
│   │   ├── media.rs            # Media service
│   │   ├── playback.rs         # Playback service
│   │   ├── sync.rs             # Sync service
│   │   ├── cache.rs            # Cache service
│   │   ├── metadata.rs         # Metadata provider service
│   │   ├── source.rs           # Source management service
│   │   └── account.rs          # Account management service
│   ├── backends/
│   │   ├── mod.rs
│   │   ├── traits.rs           # Backend trait definitions
│   │   ├── plex/
│   │   │   ├── mod.rs
│   │   │   ├── api.rs
│   │   │   ├── auth.rs
│   │   │   └── models.rs
│   │   ├── jellyfin/
│   │   │   └── ...
│   │   ├── local/
│   │   │   ├── mod.rs
│   │   │   ├── scanner.rs      # File system scanner
│   │   │   ├── matcher.rs      # File name parser & matcher
│   │   │   └── indexer.rs      # Local media indexer
│   │   └── metadata/
│   │       ├── mod.rs
│   │       ├── traits.rs       # Metadata provider traits
│   │       ├── tmdb.rs         # The Movie Database provider
│   │       ├── tvdb.rs         # TheTVDB provider
│   │       ├── omdb.rs         # Open Movie Database provider
│   │       └── aggregator.rs   # Metadata aggregation logic
│   ├── models/
│   │   ├── mod.rs
│   │   ├── media.rs            # Common media models
│   │   ├── user.rs
│   │   └── playback.rs
│   ├── state/
│   │   ├── mod.rs
│   │   ├── app_state.rs        # Global application state
│   │   ├── actions.rs          # State actions
│   │   └── reducers.rs         # State reducers
│   ├── player/
│   │   ├── mod.rs
│   │   ├── gstreamer.rs        # GStreamer integration
│   │   ├── controls.rs         # Playback controls
│   │   └── subtitles.rs        # Subtitle handling
│   └── utils/
│       ├── mod.rs
│       ├── image.rs            # Image loading/caching
│       ├── network.rs          # Network utilities
│       └── errors.rs           # Error types
├── resources/
│   ├── icons/
│   ├── ui/                     # Blueprint UI files
│   └── style.css              # Custom CSS
├── data/
│   ├── com.github.username.Reel.desktop
│   ├── com.github.username.Reel.metainfo.xml
│   └── icons/
├── po/                         # Translations
├── Cargo.toml
├── meson.build
├── PRODUCT.md
├── TECHNICAL.md
└── README.md
```

## Core Components

### 1. Source and Account Management

```rust
// Account Management - handles authentication and server discovery
#[derive(Debug, Clone)]
pub struct PlexAccount {
    pub id: String,
    pub username: String,
    pub email: String,
    pub token: String,
    pub discovered_servers: Vec<PlexServer>,
}

#[derive(Debug, Clone)]
pub struct PlexServer {
    pub machine_id: String,
    pub name: String,
    pub addresses: Vec<ServerAddress>,  // Multiple URLs to test
    pub owned: bool,
    pub shared: bool,
}

#[derive(Debug, Clone)]
pub struct ServerAddress {
    pub uri: String,
    pub is_local: bool,
    pub latency_ms: Option<u32>,  // Measured during connection test
}

pub struct AccountManager {
    plex_accounts: HashMap<String, PlexAccount>,
    jellyfin_credentials: HashMap<String, JellyfinCredentials>,
}

impl AccountManager {
    pub async fn add_plex_account(&mut self, username: &str, password: &str) -> Result<PlexAccount>;
    pub async fn discover_plex_servers(&self, account: &PlexAccount) -> Result<Vec<PlexServer>>;
    pub async fn test_server_addresses(&self, server: &PlexServer) -> Result<ServerAddress>;
    pub async fn refresh_plex_token(&mut self, account_id: &str) -> Result<()>;
}

// Source Management - represents actual media providers
#[derive(Debug, Clone)]
pub struct Source {
    pub id: String,
    pub name: String,
    pub source_type: SourceType,
    pub backend: Arc<dyn MediaBackend>,
    pub libraries: Vec<Library>,
    pub connection_info: ConnectionInfo,
    pub sync_config: SyncConfig,
}

#[derive(Debug, Clone)]
pub enum SourceType {
    PlexServer { account_id: String, machine_id: String },
    JellyfinServer { server_id: String },
    Local,
    Network { share_type: NetworkShareType },
}

#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub primary_url: String,
    pub fallback_urls: Vec<String>,
    pub last_successful_url: Option<String>,
    pub connection_tested_at: Option<DateTime<Utc>>,
}

pub struct SourceManager {
    sources: HashMap<String, Source>,
    account_manager: Arc<AccountManager>,
}

impl SourceManager {
    pub async fn create_sources_from_plex_account(&mut self, account: &PlexAccount) -> Result<Vec<Source>>;
    pub async fn add_jellyfin_source(&mut self, url: &str, credentials: JellyfinCredentials) -> Result<Source>;
    pub async fn add_local_source(&mut self, folders: Vec<PathBuf>) -> Result<Source>;
    pub async fn test_source_connectivity(&self, source_id: &str) -> Result<ConnectionStatus>;
    pub async fn get_all_libraries(&self) -> Vec<(Source, Library)>;
}
```

### 2. Backend Abstraction Layer

```rust
#[async_trait]
pub trait MediaBackend: Send + Sync {
    async fn connect(&self, connection_info: &ConnectionInfo) -> Result<()>;
    async fn get_libraries(&self) -> Result<Vec<Library>>;
    async fn get_movies(&self, library_id: &str) -> Result<Vec<Movie>>;
    async fn get_shows(&self, library_id: &str) -> Result<Vec<Show>>;
    async fn get_episodes(&self, show_id: &str, season: u32) -> Result<Vec<Episode>>;
    async fn get_stream_url(&self, media_id: &str) -> Result<StreamInfo>;
    async fn update_progress(&self, media_id: &str, position: Duration) -> Result<()>;
    
    // Backend identification
    fn backend_type(&self) -> BackendType;
    fn supports_multiple_addresses(&self) -> bool;
    fn needs_metadata_enrichment(&self) -> bool;
}

// Plex-specific backend implementation
pub struct PlexBackend {
    token: String,  // Shared across all servers from same account
    machine_id: String,
    current_url: RwLock<String>,
}

impl PlexBackend {
    pub fn new(token: String, machine_id: String) -> Self;
    pub async fn test_and_select_best_url(&self, addresses: &[ServerAddress]) -> Result<String>;
}

// Local files backend - singleton
pub struct LocalBackend {
    folders: RwLock<Vec<LocalLibrary>>,
    indexer: MediaIndexer,
    metadata_service: Arc<MetadataService>,
}

#[derive(Debug, Clone)]
pub struct LocalLibrary {
    pub path: PathBuf,
    pub name: String,
    pub media_type: MediaType,
    pub last_scan: Option<DateTime<Utc>>,
}

impl LocalBackend {
    pub fn new() -> Self;
    pub async fn add_folder(&mut self, path: PathBuf, name: String) -> Result<()>;
    pub async fn remove_folder(&mut self, path: &Path) -> Result<()>;
    pub async fn scan_all_folders(&self) -> Result<Vec<Media>>;
}
```

### 3. State Management

```rust
pub struct AppState {
    source_manager: Arc<RwLock<SourceManager>>,
    account_manager: Arc<RwLock<AccountManager>>,
    current_source: Arc<RwLock<Option<Source>>>,
    current_library: Arc<RwLock<Option<Library>>>,
    media_cache: Arc<MediaCache>,
    playback_state: Arc<RwLock<PlaybackState>>,
}

impl AppState {
    pub fn dispatch(&self, action: Action) {
        // Handle state changes
    }
    
    pub fn subscribe<F>(&self, callback: F) -> SubscriptionId 
    where 
        F: Fn(&AppState) + 'static
    {
        // Subscribe to state changes
    }
    
    pub async fn get_unified_library(&self) -> Result<Vec<MediaItem>>;
    pub async fn get_source_libraries(&self, source_id: &str) -> Result<Vec<Library>>;
}
```

### 4. Media Player

```rust
pub struct MediaPlayer {
    pipeline: gst::Pipeline,
    video_sink: gst::Element,
    state: Arc<RwLock<PlayerState>>,
}

impl MediaPlayer {
    pub fn new(video_widget: &gtk::Widget) -> Result<Self>;
    pub fn load_media(&self, url: &str) -> Result<()>;
    pub fn play(&self) -> Result<()>;
    pub fn pause(&self) -> Result<()>;
    pub fn seek(&self, position: Duration) -> Result<()>;
    pub fn set_subtitle_track(&self, index: i32) -> Result<()>;
    pub fn set_audio_track(&self, index: i32) -> Result<()>;
}
```

### 5. Metadata Provider System

```rust
#[async_trait]
pub trait MetadataProvider: Send + Sync {
    async fn search_movie(&self, title: &str, year: Option<u32>) -> Result<Vec<MovieMatch>>;
    async fn search_show(&self, title: &str, year: Option<u32>) -> Result<Vec<ShowMatch>>;
    async fn get_movie_details(&self, id: &str) -> Result<MovieMetadata>;
    async fn get_show_details(&self, id: &str) -> Result<ShowMetadata>;
    async fn get_episode_details(&self, show_id: &str, season: u32, episode: u32) -> Result<EpisodeMetadata>;
    async fn get_artwork(&self, media_id: &str) -> Result<Vec<Artwork>>;
    fn provider_name(&self) -> &str;
    fn priority(&self) -> u32;
}

pub struct MetadataService {
    providers: Vec<Arc<dyn MetadataProvider>>,
    cache: Arc<MetadataCache>,
    file_parser: FileNameParser,
}

impl MetadataService {
    pub async fn match_file(&self, file_path: &Path) -> Result<MediaMatch> {
        // Parse filename to extract title, year, season, episode
        let parsed = self.file_parser.parse(file_path)?;
        
        // Search across all providers
        let matches = self.search_all_providers(&parsed).await?;
        
        // Score and rank matches
        let best_match = self.rank_matches(matches, &parsed)?;
        
        Ok(best_match)
    }
    
    pub async fn enrich_media(&self, media: &mut Media) -> Result<()> {
        // Fetch metadata from all providers
        let metadata = self.aggregate_metadata(media).await?;
        
        // Apply metadata to media object
        media.apply_metadata(metadata)?;
        
        // Cache the enriched data
        self.cache.store(media).await?;
        
        Ok(())
    }
}

pub struct FileNameParser {
    patterns: Vec<Regex>,
}

impl FileNameParser {
    pub fn parse(&self, path: &Path) -> Result<ParsedFile> {
        // Extract title, year, season, episode from filename
        // Handle various naming conventions:
        // - "Movie Title (2023).mkv"
        // - "Show.Name.S01E05.1080p.mkv"
        // - "Show Name - 1x05 - Episode Title.mp4"
    }
}

#[derive(Debug, Clone)]
pub struct ParsedFile {
    pub title: String,
    pub year: Option<u32>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub resolution: Option<String>,
    pub media_type: MediaType,
}

#[derive(Debug, Clone)]
pub struct MediaMatch {
    pub confidence: f32,
    pub metadata: MediaMetadata,
    pub provider: String,
}
```

### 6. Sync & Cache System

```rust
pub struct SyncManager {
    cache: Arc<CacheManager>,
    sync_queue: Arc<RwLock<VecDeque<SyncTask>>>,
    sync_status: Arc<RwLock<HashMap<String, SyncStatus>>>,
}

impl SyncManager {
    pub async fn sync_source(&self, source_id: &str, source: &Source) -> Result<SyncResult>;
    pub async fn sync_library(&self, source_id: &str, library_id: &str) -> Result<()>;
    pub async fn sync_all_sources(&self, sources: &[Source]) -> Result<Vec<SyncResult>>;
    pub async fn get_sync_status(&self, source_id: &str) -> SyncStatus;
    pub async fn schedule_sync(&self, task: SyncTask);
    pub async fn cancel_sync(&self, source_id: &str);
}

#[derive(Debug, Clone)]
pub struct SyncTask {
    pub source_id: String,
    pub sync_type: SyncType,
    pub priority: SyncPriority,
    pub scheduled_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum SyncType {
    Full,           // Full sync of all data
    Incremental,    // Only changes since last sync
    Library(String), // Specific library
    Media(String),   // Specific media item
}

#[derive(Debug, Clone)]
pub enum SyncStatus {
    Idle,
    Syncing { progress: f32, current_item: String },
    Completed { at: DateTime<Utc>, items_synced: usize },
    Failed { error: String, at: DateTime<Utc> },
}

pub struct CacheManager {
    db: SqlitePool,
    image_cache: Arc<ImageCache>,
    metadata_cache: Arc<MetadataCache>,
    offline_store: Arc<OfflineStore>,
}

impl CacheManager {
    pub async fn get_or_fetch<T, F>(&self, key: &str, fetcher: F) -> Result<T>
    where
        T: Serialize + DeserializeOwned,
        F: Future<Output = Result<T>>,
    {
        // Check cache first
        if let Some(cached) = self.get_cached(key).await? {
            return Ok(cached);
        }
        
        // Try to fetch from backend
        match fetcher.await {
            Ok(data) => {
                self.set_cached(key, &data).await?;
                Ok(data)
            }
            Err(e) => {
                // If fetch fails, try offline store
                if let Some(offline) = self.offline_store.get(key).await? {
                    Ok(offline)
                } else {
                    Err(e)
                }
            }
        }
    }
    
    pub async fn store_for_offline(&self, backend_id: &str, data: &impl Serialize) -> Result<()>;
    pub async fn get_offline_data<T: DeserializeOwned>(&self, backend_id: &str) -> Result<Option<T>>;
    pub async fn clear_backend_cache(&self, backend_id: &str) -> Result<()>;
}

pub struct OfflineStore {
    db: SqlitePool,
}

impl OfflineStore {
    pub async fn store_library(&self, backend_id: &str, library: &Library) -> Result<()>;
    pub async fn store_media_batch(&self, backend_id: &str, media: &[Movie]) -> Result<()>;
    pub async fn get_libraries(&self, backend_id: &str) -> Result<Vec<Library>>;
    pub async fn get_movies(&self, backend_id: &str, library_id: &str) -> Result<Vec<Movie>>;
    pub async fn mark_for_offline(&self, media_id: &str) -> Result<()>;
    pub async fn is_available_offline(&self, media_id: &str) -> bool;
}
```

### 7. Local File Backend Implementation

```rust
pub struct LocalFileBackend {
    paths: Vec<PathBuf>,
    metadata_service: Arc<MetadataService>,
    indexer: MediaIndexer,
}

#[async_trait]
impl MediaBackend for LocalFileBackend {
    async fn get_movies(&self, library_id: &str) -> Result<Vec<Movie>> {
        // Scan directories for video files
        let files = self.indexer.scan_directory(&self.paths)?;
        
        let mut movies = Vec::new();
        for file in files {
            // Check if already indexed
            if let Some(movie) = self.indexer.get_cached(&file)? {
                movies.push(movie);
                continue;
            }
            
            // Match file with metadata providers
            let match_result = self.metadata_service.match_file(&file).await?;
            
            // Create movie with enriched metadata
            let mut movie = Movie::from_file(&file);
            movie.apply_match(match_result)?;
            
            // Cache the indexed movie
            self.indexer.cache_movie(&movie)?;
            movies.push(movie);
        }
        
        Ok(movies)
    }
    
    async fn needs_metadata_enrichment(&self) -> bool {
        true // Local files always need enrichment
    }
}

pub struct MediaIndexer {
    db: SqlitePool,
    file_watcher: FileWatcher,
}

impl MediaIndexer {
    pub fn scan_directory(&self, paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
        // Recursively scan for video files
        // Filter by video extensions
        // Check modification times for changes
    }
    
    pub async fn watch_for_changes(&self) -> Result<()> {
        // Set up file system watcher
        // Trigger re-indexing on changes
    }
}
```

### 8. UI Components

```rust
pub struct MainWindow {
    window: adw::ApplicationWindow,
    stack: gtk::Stack,
    player_view: PlayerView,
    library_view: LibraryView,
    state: Arc<AppState>,
}

impl MainWindow {
    pub fn new(app: &adw::Application, state: Arc<AppState>) -> Self {
        // Build UI
    }
    
    fn setup_actions(&self) {
        // Setup GActions
    }
    
    fn bind_state(&self) {
        // Bind state to UI
    }
}
```

## Data Models

### Common Media Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Movie {
    pub id: String,
    pub title: String,
    pub year: Option<u32>,
    pub duration: Duration,
    pub rating: Option<f32>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub overview: Option<String>,
    pub genres: Vec<String>,
    pub cast: Vec<Person>,
    pub crew: Vec<Person>,
    pub streams: Vec<StreamInfo>,
    pub external_ids: ExternalIds,
    pub metadata_source: MetadataSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalIds {
    pub tmdb_id: Option<String>,
    pub tvdb_id: Option<String>,
    pub imdb_id: Option<String>,
    pub omdb_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetadataSource {
    Plex,
    Jellyfin,
    TMDB,
    TVDB,
    OMDB,
    Manual,
    Composite(Vec<String>), // Multiple sources
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Show {
    pub id: String,
    pub title: String,
    pub year: Option<u32>,
    pub seasons: Vec<Season>,
    pub rating: Option<f32>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub overview: Option<String>,
    pub genres: Vec<String>,
    pub cast: Vec<Person>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    pub url: String,
    pub direct_play: bool,
    pub video_codec: String,
    pub audio_codec: String,
    pub container: String,
    pub bitrate: u64,
    pub resolution: Resolution,
}
```

## Database Schema

```sql
-- Media cache with source support
CREATE TABLE media_cache (
    id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL,
    library_id TEXT NOT NULL,
    type TEXT NOT NULL,
    data JSON NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    synced_at TIMESTAMP,
    expires_at TIMESTAMP,
    metadata_source TEXT,
    external_ids JSON,
    INDEX idx_source_library (source_id, library_id),
    INDEX idx_source_type (source_id, type)
);

-- Offline storage for source data
CREATE TABLE offline_store (
    id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL,
    library_id TEXT NOT NULL,
    media_type TEXT NOT NULL,
    data JSON NOT NULL,
    stored_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_accessed TIMESTAMP,
    is_pinned BOOLEAN DEFAULT FALSE,
    INDEX idx_source_library (source_id, library_id)
);

-- Sync metadata
CREATE TABLE sync_metadata (
    source_id TEXT PRIMARY KEY,
    last_full_sync TIMESTAMP,
    last_incremental_sync TIMESTAMP,
    total_items INTEGER,
    sync_status TEXT,
    error_message TEXT
);

-- Source configuration
CREATE TABLE source_config (
    source_id TEXT PRIMARY KEY,
    source_name TEXT NOT NULL,
    source_type TEXT NOT NULL, -- 'plex_server', 'jellyfin_server', 'local', 'network'
    account_id TEXT, -- References account that owns this source (for Plex)
    connection_info JSON NOT NULL, -- URLs, paths, etc.
    is_active BOOLEAN DEFAULT TRUE,
    auto_sync BOOLEAN DEFAULT TRUE,
    sync_interval INTEGER DEFAULT 3600,
    offline_enabled BOOLEAN DEFAULT TRUE
);

-- Account storage (Plex accounts, etc.)
CREATE TABLE accounts (
    account_id TEXT PRIMARY KEY,
    account_type TEXT NOT NULL, -- 'plex', 'jellyfin'
    username TEXT,
    email TEXT,
    token TEXT, -- Stored encrypted in keyring
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_used TIMESTAMP
);

-- Libraries within sources
CREATE TABLE libraries (
    library_id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL,
    library_name TEXT NOT NULL,
    library_type TEXT NOT NULL, -- 'movies', 'shows', 'music', 'photos'
    path TEXT, -- For local libraries
    last_synced TIMESTAMP,
    item_count INTEGER,
    FOREIGN KEY (source_id) REFERENCES source_config(source_id)
);

-- Playback progress
CREATE TABLE playback_progress (
    media_id TEXT,
    source_id TEXT,
    position INTEGER NOT NULL,
    duration INTEGER NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    synced BOOLEAN DEFAULT FALSE,
    PRIMARY KEY (media_id, source_id)
);

-- User preferences
CREATE TABLE preferences (
    key TEXT PRIMARY KEY,
    value JSON NOT NULL
);

-- Image cache metadata
CREATE TABLE image_cache (
    url TEXT PRIMARY KEY,
    source_id TEXT,
    file_path TEXT NOT NULL,
    size INTEGER NOT NULL,
    accessed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    is_offline BOOLEAN DEFAULT FALSE
);

-- Download queue for offline content
CREATE TABLE download_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    media_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    priority INTEGER DEFAULT 0,
    status TEXT DEFAULT 'pending',
    progress REAL DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    error_message TEXT,
    UNIQUE(media_id, source_id)
);

-- Local file index
CREATE TABLE local_file_index (
    file_path TEXT PRIMARY KEY,
    media_id TEXT,
    file_size INTEGER NOT NULL,
    modified_at TIMESTAMP NOT NULL,
    indexed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    metadata_matched BOOLEAN DEFAULT FALSE,
    match_confidence REAL,
    manual_override BOOLEAN DEFAULT FALSE,
    INDEX idx_media_id (media_id)
);

-- Metadata provider cache
CREATE TABLE metadata_cache (
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL,
    query_type TEXT NOT NULL, -- 'movie', 'show', 'episode'
    query_params JSON NOT NULL,
    response_data JSON NOT NULL,
    fetched_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP,
    INDEX idx_provider_query (provider, query_type)
);

-- Manual metadata corrections
CREATE TABLE metadata_corrections (
    media_id TEXT PRIMARY KEY,
    original_match JSON NOT NULL,
    corrected_match JSON NOT NULL,
    corrected_by TEXT,
    corrected_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

## Network Architecture

### API Communication
- REST API for Plex/Jellyfin
- REST API for metadata providers (TMDB, TVDB, OMDB)
- WebSocket for real-time updates (future)
- HTTP/2 with connection pooling
- Automatic retry with exponential backoff
- Request caching and deduplication
- Rate limiting per provider
- Offline detection and fallback
- Smart sync scheduling based on network conditions

### Media Streaming
- Direct play when possible
- Transcoding fallback
- Adaptive bitrate streaming
- Bandwidth detection
- Resume capability
- Offline playback for downloaded content
- Progressive download with play-while-downloading

### Sync Strategy
```rust
pub struct SyncStrategy {
    // Sync intervals
    pub full_sync_interval: Duration,      // Default: 24 hours
    pub incremental_sync_interval: Duration, // Default: 1 hour
    pub on_demand_sync: bool,              // Sync when opening library
    
    // Network conditions
    pub wifi_only: bool,                   // Only sync on WiFi
    pub metered_connection_limit: usize,   // MB limit on metered connections
    
    // Content strategy
    pub auto_download_next_episodes: bool,
    pub keep_watched_items_days: u32,
    pub max_offline_storage_gb: u32,
}
```

## Security Considerations

### Authentication
- OAuth2 for Plex
- API key storage in system keyring
- Session management
- Automatic token refresh

### Data Protection
- Credentials stored in Secret Service
- Cache encryption for sensitive data
- Secure communication (HTTPS only)
- Input validation and sanitization

## Performance Optimizations

### UI Performance
- Virtual scrolling for large lists
- Image lazy loading
- Debounced search
- Optimistic UI updates
- Background data fetching

### Memory Management
- Image cache with LRU eviction
- Metadata cache limits
- Stream buffering controls
- Periodic cache cleanup

### Network Performance
- Connection pooling
- Request batching
- Progressive image loading
- Predictive prefetching
- CDN support

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_backend_authentication() {
        // Test authentication flow
    }
    
    #[tokio::test]
    async fn test_media_fetching() {
        // Test media API calls
    }
}
```

### Integration Tests
- Backend API integration
- Database operations
- Cache functionality
- State management

### UI Tests
- Component rendering
- User interactions
- Navigation flows
- Accessibility

## Build and Deployment

### Development Build
```bash
cargo build
cargo run
```

### Release Build
```bash
cargo build --release
meson setup build
ninja -C build
```

### Flatpak Build
```bash
flatpak-builder --repo=repo build-dir com.github.username.Reel.json
```

## Configuration

### Settings Schema
```toml
[general]
theme = "auto"  # auto|light|dark
language = "system"

[metadata]
enabled = true
auto_match = true
match_confidence_threshold = 0.8
providers = ["tmdb", "tvdb", "omdb"]

[metadata.tmdb]
api_key = ""  # Stored in keyring
enabled = true
priority = 1
cache_duration = 604800  # 7 days in seconds

[metadata.tvdb]
api_key = ""  # Stored in keyring
enabled = true
priority = 2
cache_duration = 604800

[metadata.omdb]
api_key = ""  # Stored in keyring
enabled = false
priority = 3
cache_duration = 2592000  # 30 days

[playback]
hardware_acceleration = true
default_subtitle = "none"
default_audio = "original"
skip_intro = true

[network]
connection_timeout = 30
max_retries = 3
cache_size = 1000  # MB

[sync]
enabled = true
auto_sync = true
wifi_only = false
full_sync_interval = 86400  # seconds (24 hours)
incremental_sync_interval = 3600  # seconds (1 hour)
max_offline_storage = 10  # GB
auto_cleanup = true
keep_watched_days = 7

[offline]
enabled = true
auto_download_next = true
max_concurrent_downloads = 2
download_quality = "original"  # original|high|medium|low

# Account configurations
[[accounts]]
id = "plex_personal"
type = "plex"
username = "john.doe@example.com"
# Token stored in keyring

[[accounts]]
id = "plex_family"
type = "plex"
username = "family@example.com"
# Token stored in keyring

# Source configurations (auto-discovered from accounts + manually added)
[[sources]]
id = "plex_home_server"
name = "Home Server"
type = "plex_server"
account_id = "plex_personal"
machine_id = "abc123..."
primary_url = "https://192.168.1.100:32400"
fallback_urls = ["https://home.example.com:32400", "http://192.168.1.100:32400"]
auto_sync = true
offline_enabled = true

[[sources]]
id = "plex_shared_server"
name = "Friend's Server"
type = "plex_server"
account_id = "plex_personal"
machine_id = "def456..."
primary_url = "https://friend.example.com:32400"
auto_sync = true
offline_enabled = false

[[sources]]
id = "jellyfin_main"
name = "Main Jellyfin"
type = "jellyfin_server"
server_url = "https://jellyfin.example.com"
username = "jellyfin_user"
# Password stored in keyring
auto_sync = true
offline_enabled = false

[[sources]]
id = "local_files"
name = "Local Media"
type = "local"
auto_sync = true
offline_enabled = true  # Always available offline

# Local library folders
[[sources.libraries]]
source_id = "local_files"
path = "/home/user/Videos"
name = "Personal Videos"
media_type = "mixed"

[[sources.libraries]]
source_id = "local_files"
path = "/media/nas/movies"
name = "NAS Movies"
media_type = "movies"

[[sources.libraries]]
source_id = "local_files"
path = "/media/nas/tvshows"
name = "NAS TV Shows"
media_type = "shows"
```

## API Documentation

### Public API
```rust
/// Main application entry point
pub struct ReelApp {
    // ...
}

impl ReelApp {
    /// Create new application instance
    pub fn new() -> Self;
    
    /// Run the application
    pub fn run(&self) -> Result<()>;
    
    /// Account operations
    pub async fn add_plex_account(&self, username: &str, password: &str) -> Result<Vec<Source>>;
    pub async fn remove_account(&self, account_id: &str) -> Result<()>;
    pub async fn refresh_account_servers(&self, account_id: &str) -> Result<Vec<Source>>;
    
    /// Source operations
    pub async fn add_jellyfin_source(&self, url: &str, username: &str, password: &str) -> Result<Source>;
    pub async fn add_local_folder(&self, path: PathBuf, name: String) -> Result<()>;
    pub async fn remove_source(&self, source_id: &str) -> Result<()>;
    pub async fn test_source_connectivity(&self, source_id: &str) -> Result<ConnectionStatus>;
    
    /// Library operations
    pub async fn get_unified_library(&self) -> Result<Vec<MediaItem>>;
    pub async fn get_source_libraries(&self, source_id: &str) -> Result<Vec<Library>>;
    
    /// Sync operations
    pub async fn sync_all_sources(&self) -> Result<Vec<SyncResult>>;
    pub async fn sync_source(&self, source_id: &str) -> Result<SyncResult>;
    pub async fn refresh_library(&self, source_id: &str, library_id: &str) -> Result<()>;
    
    /// Offline operations
    pub async fn download_for_offline(&self, media_id: &str, source_id: &str) -> Result<()>;
    pub async fn remove_offline(&self, media_id: &str) -> Result<()>;
    pub async fn get_offline_status(&self) -> OfflineStatus;
    
    /// Metadata operations
    pub async fn match_local_file(&self, file_path: &Path) -> Result<MediaMatch>;
    pub async fn override_metadata(&self, media_id: &str, metadata: MediaMetadata) -> Result<()>;
    pub async fn refresh_metadata(&self, media_id: &str) -> Result<()>;
    pub async fn get_alternative_matches(&self, media_id: &str) -> Result<Vec<MediaMatch>>;
}

/// Sync result information
#[derive(Debug, Clone)]
pub struct SyncResult {
    pub source_id: String,
    pub source_name: String,
    pub success: bool,
    pub items_synced: usize,
    pub duration: Duration,
    pub errors: Vec<String>,
}

/// Offline storage status
#[derive(Debug, Clone)]
pub struct OfflineStatus {
    pub total_size_mb: u64,
    pub used_size_mb: u64,
    pub items_count: usize,
    pub sources: HashMap<String, SourceOfflineInfo>,
}

/// Connection status for a source
#[derive(Debug, Clone)]
pub struct ConnectionStatus {
    pub is_online: bool,
    pub selected_url: Option<String>,
    pub latency_ms: Option<u32>,
    pub last_error: Option<String>,
}
```

## Future Considerations

### Planned Features
- Plugin system using WASM
- Remote control support
- Enhanced sync with selective library sync
- Companion app communication
- Media server capabilities
- Conflict resolution for multi-device sync
- Smart caching based on viewing patterns
- Peer-to-peer sync between devices

### Scalability
- Multi-server support
- Large library optimization (100k+ items)
- Distributed caching
- Background sync workers

### Platform Expansion
- Adaptive UI for mobile (libadwaita)
- Elementary OS variant
- KDE/Plasma version (future)
- Web UI (WASM target)

## Dependencies

### Core Dependencies
```toml
[dependencies]
gtk4 = "0.7"
libadwaita = "0.5"
gstreamer = "0.21"
tokio = { version = "1.35", features = ["full"] }
reqwest = { version = "0.11", features = ["json", "stream"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
sqlx = { version = "0.7", features = ["sqlite", "runtime-tokio"] }
anyhow = "1.0"
thiserror = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"
once_cell = "1.19"
async-trait = "0.1"
url = "2.5"
chrono = "0.4"
dirs = "5.0"
keyring = "2.0"
image = "0.24"

# File parsing and matching
regex = "1.10"
fuzzy-matcher = "0.3"
levenshtein = "1.0"
walkdir = "2.4"
notify = "6.1"  # File system watching

# Metadata providers (example crates, actual may vary)
tmdb-api = "0.4"  # TMDB API client
# Custom implementations for TVDB and OMDB
```

## Development Guidelines

### Code Style
- Follow Rust standard style (rustfmt)
- Use clippy for linting
- Comprehensive documentation
- Example code in docs

### Git Workflow
- Feature branches
- Conventional commits
- PR reviews required
- CI must pass

### Release Process
1. Version bump in Cargo.toml
2. Update changelog
3. Tag release
4. Build packages
5. Publish to Flathub

## Monitoring and Logging

### Logging Levels
- ERROR: Critical failures
- WARN: Recoverable issues
- INFO: Important events
- DEBUG: Detailed debugging
- TRACE: Verbose tracing

### Metrics
- Application performance
- API response times
- Cache hit rates
- Playback statistics
- Error rates

## Sync & Offline Usage Patterns

### Typical User Flows

#### Initial Setup
1. User adds a backend (Plex/Jellyfin server)
2. App performs initial full sync
3. Libraries and metadata cached locally
4. Thumbnails downloaded in background
5. UI immediately available with cached data

#### Daily Usage (Online)
1. App starts with cached data (instant)
2. Background incremental sync begins
3. UI updates with any changes
4. User can browse/play immediately
5. Sync status indicator shows progress

#### Offline Usage
1. App detects offline state
2. All UI remains functional with cached data
3. Only cached/downloaded media can play
4. Changes queued for later sync
5. Visual indicators show offline mode

#### Smart Sync Examples
```rust
// Automatic sync on app launch
app.on_startup(|state| {
    if state.network_available() && state.time_since_last_sync() > Duration::hours(1) {
        state.sync_all_backends_incremental();
    }
});

// Sync when entering a library
library_view.on_enter(|backend_id, library_id| {
    if state.should_sync(backend_id) {
        state.sync_library(backend_id, library_id);
    }
});

// Download next episodes automatically
player.on_episode_complete(|episode| {
    if settings.auto_download_next {
        let next = episode.get_next();
        offline_manager.queue_download(next, Priority::High);
    }
});
```

## Conclusion

This technical architecture provides a solid foundation for building a modern, performant media player for GNOME. The comprehensive sync and offline support ensures users always have access to their media libraries, whether online or offline. The modular design allows for easy extension and maintenance while the abstracted backend system ensures flexibility for future media sources.