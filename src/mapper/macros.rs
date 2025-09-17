//! Macros for automatic field mapping generation

/// Macro for implementing automatic field-by-field mapping
///
/// # Examples
///
/// ```rust
/// map_struct! {
///     MovieEntity => Movie {
///         id => id,
///         title => title,
///         year => year.map(|y| y as u32),
///         duration_ms => duration.map(|ms| Duration::from_millis(ms as u64)),
///         rating => rating,
///         poster_url => poster_url,
///         backdrop_url => backdrop_url,
///         overview => overview,
///         genres => genres.and_then(|g| serde_json::from_value(g).ok()).unwrap_or_default(),
///     }
/// }
/// ```
#[macro_export]
macro_rules! map_struct {
    (
        $from:ty => $to:ty {
            $($from_field:ident => $to_expr:expr),* $(,)?
        }
    ) => {
        impl From<$from> for $to {
            fn from(from: $from) -> Self {
                Self {
                    $($from_field: {
                        let value = from.$from_field;
                        $to_expr
                    },)*
                }
            }
        }
    };
}

/// Macro for implementing fallible field-by-field mapping
///
/// # Examples
///
/// ```rust
/// try_map_struct! {
///     MediaItemModel => MediaItem {
///         // Simple field mappings
///         id => id,
///         title => title,
///
///         // Complex transformations with error handling
///         metadata => {
///             metadata.as_ref()
///                 .and_then(|json| serde_json::from_value(json.clone()).ok())
///                 .ok_or_else(|| anyhow!("Failed to parse metadata"))?
///         },
///
///         // Conditional mapping based on type
///         media_type => match media_type.as_str() {
///             "movie" => MediaItem::Movie(/* ... */),
///             "show" => MediaItem::Show(/* ... */),
///             _ => return Err(anyhow!("Unknown media type")),
///         }
///     }
/// }
/// ```
#[macro_export]
macro_rules! try_map_struct {
    (
        $from:ty => $to:ty {
            $($from_field:ident => $to_expr:expr),* $(,)?
        }
    ) => {
        impl TryFrom<$from> for $to {
            type Error = anyhow::Error;

            fn try_from(from: $from) -> Result<Self, Self::Error> {
                Ok(Self {
                    $($from_field: {
                        let value = from.$from_field;
                        $to_expr
                    },)*
                })
            }
        }
    };
}

/// Macro for defining bidirectional mapping between types
///
/// # Examples
///
/// ```rust
/// bidirectional_map! {
///     SourceId, String {
///         forward: |s: SourceId| s.0,
///         backward: |s: String| SourceId(s),
///     }
/// }
/// ```
#[macro_export]
macro_rules! bidirectional_map {
    (
        $type_a:ty, $type_b:ty {
            forward: $forward:expr,
            backward: $backward:expr $(,)?
        }
    ) => {
        impl From<$type_a> for $type_b {
            fn from(value: $type_a) -> Self {
                ($forward)(value)
            }
        }

        impl From<$type_b> for $type_a {
            fn from(value: $type_b) -> Self {
                ($backward)(value)
            }
        }
    };
}

/// Macro for mapping enum variants
///
/// # Examples
///
/// ```rust
/// map_enum! {
///     MediaType => String {
///         Movie => "movie",
///         Show => "show",
///         Episode => "episode",
///         MusicAlbum => "album",
///         MusicTrack => "track",
///         Photo => "photo",
///     }
/// }
/// ```
#[macro_export]
macro_rules! map_enum {
    (
        $enum_type:ty => $target_type:ty {
            $($variant:ident => $value:expr),* $(,)?
        }
    ) => {
        impl From<$enum_type> for $target_type {
            fn from(value: $enum_type) -> Self {
                match value {
                    $(<$enum_type>::$variant => $value,)*
                }
            }
        }
    };
}

/// Macro for nested object mapping
///
/// # Examples
///
/// ```rust
/// nested_map! {
///     MovieWithLibrary {
///         movie: Movie => MovieEntity,
///         library: Library => LibraryEntity,
///     }
/// }
/// ```
#[macro_export]
macro_rules! nested_map {
    (
        $struct_name:ident {
            $($field:ident: $from:ty => $to:ty),* $(,)?
        }
    ) => {
        impl $struct_name {
            pub fn map_nested(self) -> Result<($($to,)*), anyhow::Error> {
                Ok((
                    $(
                        <$to>::try_from(self.$field)?
                    ,)*
                ))
            }
        }
    };
}

/// Macro for creating a field transformer
///
/// # Examples
///
/// ```rust
/// field_transformer! {
///     DurationTransformer: i64 => Duration {
///         |ms| Duration::from_millis(ms as u64)
///     }
/// }
/// ```
#[macro_export]
macro_rules! field_transformer {
    (
        $name:ident: $from:ty => $to:ty {
            $transform:expr
        }
    ) => {
        pub struct $name;

        impl crate::mapper::FieldTransform<$from, $to> for $name {
            fn transform(source: $from) -> $to {
                ($transform)(source)
            }
        }
    };
}
