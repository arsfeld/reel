pub mod autolayout;
pub mod colors;
pub mod image_cache;

pub use autolayout::{AutoLayout, NSEdgeInsets};
pub use colors::ReelColors;
pub use image_cache::{ImageCache, get_image_cache};

// Re-export geometry types from objc2-foundation
pub use objc2_foundation::{NSPoint as CGPoint, NSRect as CGRect, NSSize as CGSize};

// CGFloat is a type alias that depends on architecture
#[cfg(target_pointer_width = "64")]
pub type CGFloat = f64;

#[cfg(target_pointer_width = "32")]
pub type CGFloat = f32;

use objc2::MainThreadMarker;

/// Get the main thread marker for creating NSObjects
pub fn main_thread_marker() -> MainThreadMarker {
    // SAFETY: We assume this is called from the main thread
    // In a real application, you should ensure this is only called from the main thread
    unsafe { MainThreadMarker::new_unchecked() }
}
