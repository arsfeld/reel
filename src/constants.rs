// Performance tuning constants - adjust these to balance performance vs responsiveness
// All performance-related constants in one place for easy tuning

// === UI Performance ===
// Library view performance constants
#[allow(dead_code)]
pub const SCROLL_DEBOUNCE_MS: u64 = 100; // Debounce scroll events to reduce image loading
#[allow(dead_code)]
pub const IMAGE_VIEWPORT_BUFFER: f64 = 200.0; // Buffer area around viewport for image loading
#[allow(dead_code)]
pub const CARD_BATCH_SIZE: usize = 20; // Number of cards to load in each batch

// Player UI constants
pub const PLAYER_CONTROLS_HIDE_DELAY_SECS: u64 = 2; // Seconds before hiding player controls on inactivity

// Homepage specific constants
pub const HOME_INITIAL_CARDS_PER_SECTION: usize = 12; // Initial cards per section on homepage
pub const HOME_INITIAL_IMAGES_PER_SECTION: usize = 10; // Initial images to load per section

// === Network Performance ===
// Removed unused HTTP and caching constants
// These were for features not yet implemented

// === Virtual Scrolling ===
// Virtual scrolling support removed - always use standard library view
// Removed USE_VIRTUAL_SCROLLING and VIRTUAL_SCROLL_THRESHOLD constants
