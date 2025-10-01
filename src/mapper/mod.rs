//! Type-safe object mapper for model conversions
//!
//! This module provides a declarative, type-safe way to map between database entities
//! and domain models, reducing boilerplate and potential for errors.

pub mod macros;
pub mod media_item_mapper;

#[cfg(test)]
mod tests;
