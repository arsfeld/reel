//! Type-safe object mapper for model conversions
//!
//! This module provides a declarative, type-safe way to map between database entities
//! and domain models, reducing boilerplate and potential for errors.

pub mod macros;
pub mod media_item_mapper;
pub mod traits;

pub use macros::*;
pub use media_item_mapper::{DateTimeTransformer, DurationTransformer, JsonTransformer};
pub use traits::{FieldTransform, Mapper, TryMapper};

#[cfg(test)]
mod tests;
