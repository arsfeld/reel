//! Core mapper traits for type conversions

use std::convert::{From, TryFrom};

/// Trait for infallible mapping between types
pub trait Mapper<T>: Sized {
    /// Map from another type to Self
    fn map_from(value: T) -> Self;

    /// Map from Self to another type
    fn map_into(self) -> T;
}

/// Trait for fallible mapping between types
pub trait TryMapper<T>: Sized {
    type Error;

    /// Try to map from another type to Self
    fn try_map_from(value: T) -> Result<Self, Self::Error>;

    /// Try to map from Self to another type
    fn try_map_into(self) -> Result<T, Self::Error>;
}

/// Trait for custom field transformations
pub trait FieldTransform<S, T> {
    /// Transform a field from source type to target type
    fn transform(source: S) -> T;
}

/// Default implementation for types that implement From
impl<S, T> Mapper<T> for S
where
    S: From<T>,
    T: From<S>,
{
    fn map_from(value: T) -> Self {
        S::from(value)
    }

    fn map_into(self) -> T {
        T::from(self)
    }
}

/// Default implementation for types that implement TryFrom
impl<S, T> TryMapper<T> for S
where
    S: TryFrom<T>,
    T: TryFrom<S>,
    <S as TryFrom<T>>::Error: std::fmt::Debug,
    <T as TryFrom<S>>::Error: std::fmt::Debug,
{
    type Error = anyhow::Error;

    fn try_map_from(value: T) -> Result<Self, Self::Error> {
        S::try_from(value).map_err(|e| anyhow::anyhow!("{:?}", e))
    }

    fn try_map_into(self) -> Result<T, Self::Error> {
        T::try_from(self).map_err(|e| anyhow::anyhow!("{:?}", e))
    }
}

/// Helper for mapping Option types
pub fn map_option<S, T, F>(opt: Option<S>, f: F) -> Option<T>
where
    F: FnOnce(S) -> T,
{
    opt.map(f)
}

/// Helper for mapping Vec types
pub fn map_vec<S, T, F>(vec: Vec<S>, f: F) -> Vec<T>
where
    F: Fn(S) -> T,
{
    vec.into_iter().map(f).collect()
}

/// Helper for mapping Result types
pub fn try_map_option<S, T, E, F>(opt: Option<S>, f: F) -> Result<Option<T>, E>
where
    F: FnOnce(S) -> Result<T, E>,
{
    match opt {
        Some(val) => f(val).map(Some),
        None => Ok(None),
    }
}

/// Helper for mapping Vec types with error handling
pub fn try_map_vec<S, T, E, F>(vec: Vec<S>, f: F) -> Result<Vec<T>, E>
where
    F: Fn(S) -> Result<T, E>,
{
    vec.into_iter().map(f).collect()
}
