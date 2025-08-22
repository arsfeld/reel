// Performance tuning constants - adjust these to balance performance vs responsiveness
// All performance-related constants in one place for easy tuning

// === UI Performance ===
// Card creation and image loading - Optimized for fast Plex transcoded thumbnails
pub const INITIAL_CARDS_TO_CREATE: usize = 60; // Number of cards to create initially (doubled)
pub const INITIAL_IMAGES_TO_LOAD: usize = 48; // Number of images to load initially (doubled)
pub const INITIAL_LOAD_DELAY_MS: u64 = 20; // Delay before initial image load (ms) (reduced)
pub const SCROLL_DEBOUNCE_MS: u64 = 30; // Delay for scroll event debouncing (ms) (halved)
pub const CARD_BATCH_SIZE: usize = 48; // Max cards to create in one batch (doubled)
pub const IMAGE_VIEWPORT_BUFFER: f64 = 600.0; // Pixels to load outside viewport (doubled)

// Player UI constants
pub const PLAYER_CONTROLS_HIDE_DELAY_SECS: u64 = 2; // Seconds before hiding player controls on inactivity

// Homepage specific constants
pub const HOME_INITIAL_CARDS_PER_SECTION: usize = 12; // Initial cards per section on homepage
pub const HOME_INITIAL_IMAGES_PER_SECTION: usize = 10; // Initial images to load per section

// === Network Performance ===
// Image loader and HTTP settings - Increased for smaller images
pub const CONCURRENT_DOWNLOADS: usize = 12; // Max concurrent image downloads (doubled for smaller images)
pub const HTTP_TIMEOUT_SECS: u64 = 10; // HTTP request timeout
pub const HTTP_CONNECT_TIMEOUT_SECS: u64 = 5; // HTTP connection timeout

// === Memory Management ===
// Cache settings
pub const MEMORY_CACHE_SIZE: usize = 1000; // Number of images in memory cache
pub const MEMORY_CACHE_MAX_MB: u64 = 500; // Max memory cache size in MB

// === API Performance ===
// Caching durations to reduce API calls
pub const HOME_SECTIONS_CACHE_SECS: u64 = 300; // Cache homepage sections for 5 minutes
