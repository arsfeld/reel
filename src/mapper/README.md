# Object Mapper Guide

## Overview

The `mapper` module provides a type-safe, declarative way to map between database entities and domain models in Reel. It reduces boilerplate code and potential for errors by providing a structured approach to object conversions.

## Key Components

### Core Traits

1. **`Mapper<T>`** - For infallible mappings between types
2. **`TryMapper<T>`** - For fallible mappings that can return errors
3. **`FieldTransform<S, T>`** - For custom field-level transformations

### Helper Functions

- `map_option()` - Maps Option types
- `map_vec()` - Maps Vec types
- `try_map_option()` - Maps Option types with error handling
- `try_map_vec()` - Maps Vec types with error handling

### Macros

The module provides several macros to reduce boilerplate:

#### `map_struct!`

For simple field-by-field mappings:

```rust
map_struct! {
    SourceType => TargetType {
        field1 => field1,
        field2 => field2.map(|x| x * 2),
        field3 => transform_field3(field3),
    }
}
```

#### `try_map_struct!`

For fallible mappings with error handling:

```rust
try_map_struct! {
    SourceType => TargetType {
        field1 => field1,
        field2 => parse_field2(field2)?,
        field3 => field3.ok_or_else(|| anyhow!("Missing field3"))?,
    }
}
```

#### `bidirectional_map!`

For types that can be converted in both directions:

```rust
bidirectional_map! {
    TypeA <=> TypeB {
        forward: |a: TypeA| TypeB::from(a),
        backward: |b: TypeB| TypeA::from(b),
    }
}
```

#### `map_enum!`

For enum variant mappings:

```rust
map_enum! {
    MediaType => String {
        Movie => "movie",
        Show => "show",
        Episode => "episode",
    }
}
```

## MediaItem Mapping

The primary use case is mapping between `MediaItem` (domain model) and `MediaItemModel` (database entity).

### Domain to Database

```rust
let movie = MediaItem::Movie(movie_data);
let db_model = movie.to_model("source-id", Some("library-id".to_string()));
```

### Database to Domain

```rust
let db_model: MediaItemModel = // ... from database
let media_item = MediaItem::try_from(db_model)?;
```

## Custom Transformers

The module includes specialized transformers for common conversions:

### DurationTransformer

Converts between Rust `Duration` and milliseconds:

```rust
let duration = DurationTransformer::from_millis(Some(5000)); // 5 seconds
let millis = DurationTransformer::to_millis(duration); // 5000
```

### DateTimeTransformer

Handles datetime conversions:

```rust
let datetime = DateTimeTransformer::from_rfc3339(Some("2024-01-01T00:00:00Z"));
let utc_datetime = DateTimeTransformer::from_naive(Some(naive_datetime));
```

### JsonTransformer

Extracts and deserializes fields from JSON metadata:

```rust
let cast: Vec<Person> = JsonTransformer::extract(&metadata, "cast").unwrap_or_default();
let genres = JsonTransformer::extract_genres(&genres_json);
```

## Migration Guide

### Before (Manual Conversion)

```rust
// Old way - manual field-by-field conversion
fn convert_media_item_to_entity(
    item: MediaItem,
    library_id: &LibraryId,
    source_id: &SourceId,
) -> Result<MediaItemModel> {
    let (title, year, duration_ms, rating, ...) = match &item {
        MediaItem::Movie(movie) => (
            movie.title.clone(),
            movie.year.map(|y| y as i32),
            Some(movie.duration.as_millis() as i64),
            // ... dozens more lines
        ),
        // ... more match arms
    };

    Ok(MediaItemModel {
        id: item.id().to_string(),
        source_id: source_id.to_string(),
        // ... many more fields
    })
}
```

### After (Using Mapper)

```rust
// New way - using the mapper
let db_model = media_item.to_model(source_id, library_id);
```

## Benefits

1. **Type Safety** - Compile-time validation of mappings
2. **Less Boilerplate** - Macros generate repetitive code
3. **Consistency** - Single source of truth for conversions
4. **Maintainability** - Easier to update when models change
5. **Error Handling** - Structured approach to handling conversion failures
6. **Testing** - Centralized conversion logic is easier to test

## Usage Examples

### Basic Conversion

```rust
use crate::mapper::media_item_mapper::*;

// Convert domain model to database entity
let movie = MediaItem::Movie(movie_data);
let db_model = movie.to_model("source-1", Some("library-1".to_string()));

// Save to database
media_repository.insert(db_model).await?;
```

### Batch Processing

```rust
// Convert multiple items
let db_models: Vec<MediaItemModel> = media_items
    .into_iter()
    .map(|item| item.to_model(source_id, Some(library_id.clone())))
    .collect();

// Batch insert
media_repository.batch_insert(db_models).await?;
```

### Error Handling

```rust
// Convert from database with error handling
match MediaItem::try_from(db_model) {
    Ok(media_item) => {
        // Process the media item
    }
    Err(e) => {
        warn!("Failed to convert model: {}", e);
        // Handle the error appropriately
    }
}
```

## Testing

The mapper includes comprehensive tests covering:

- Round-trip conversions (domain → database → domain)
- All media types (Movie, Show, Episode, Album, Track, Photo)
- Error cases (unknown media types)
- Field transformations (durations, dates, JSON)

Run tests with:

```bash
cargo test mapper::tests
```

## Future Enhancements

Potential improvements to the mapper system:

1. **Derive Macro** - Auto-generate mappers using proc macros
2. **Validation** - Add field-level validation during conversion
3. **Performance** - Optimize for large batch conversions
4. **Versioning** - Support for different model versions
5. **Partial Updates** - Map only changed fields