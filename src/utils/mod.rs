pub mod image_loader;

#[cfg(feature = "gtk")]
pub use image_loader::{ImageLoader, ImageSize};
