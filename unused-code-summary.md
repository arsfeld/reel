# Summary: Identifying and Removing Real Unused Code

## Key Discoveries

### 1. Module Visibility Was The Main Issue
- **Problem**: Modules in `lib.rs` were private (`mod` instead of `pub mod`)
- **Impact**: Made all module contents appear "unused" to compiler
- **Fix**: Changed to `pub mod` in lib.rs
- **Result**: Reduced warnings from 424 → 234 instantly!

### 2. Trait Methods Can Be Genuinely Unused
Even when a trait is actively used, individual methods might never be called:

**Removed from MediaBackend trait:**
- `mark_watched` / `mark_unwatched` - Never called
- `get_watch_status` - Never called
- `search` - Never called
- `fetch_episode_markers` / `fetch_media_markers` - Never called
- `find_next_episode` - Never called
- `get_library_items` - Never called
- `get_music_albums` / `get_music_tracks` / `get_photos` - Never called
- `get_backend_info` - Never called
- `get_last_sync_time` / `supports_offline` - Never called
- `is_initialized` / `is_playback_ready` - Never called

**Result**: Removed 16+ trait methods and their implementations across all backends!

### 3. Internal vs External Usage Matters
- `PlexBackend::is_initialized()` was used internally but not part of trait
- **Solution**: Moved to private method in `impl PlexBackend` block
- **Lesson**: Check if "unused" methods are actually used internally

### 4. Features Can Hide Usage
- Removed `relm4` feature since it was always enabled (in default features)
- Made dependencies non-optional to reflect reality
- **Result**: Cleaner, more honest dependency declaration

## Final Statistics

| Stage | Warnings | Errors | Action Taken |
|-------|----------|--------|--------------|
| Initial | 424 | 0 | Baseline |
| Fixed visibility | 234 | 0 | Made modules public in lib.rs |
| Removed trait methods | 238 | 20+ | Removed unused MediaBackend methods |
| Fixed implementations | 238 | 0 | Removed implementations from all backends |

**Total Reduction**: 424 → 238 warnings (44% reduction!)

## How to Identify Real Unused Code

### ✅ ALWAYS Check Build Errors First
```bash
cargo check 2>&1 | grep "error"
```
Never just count warnings - errors are critical!

### ✅ Fix Visibility Before Analyzing
```rust
// lib.rs - Make modules public if they should be
pub mod backends;  // Not: mod backends;
```

### ✅ Check Actual Usage, Not Just Definitions
```bash
# Find if a trait method is actually called
grep -r "\.method_name(" src --include="*.rs" | \
  grep -v "impl TraitName"
```

### ✅ Distinguish Internal vs External Usage
- Method used internally? Move to private impl block
- Method never used? Remove completely

### ✅ Consider Test Usage
- Tests need public visibility
- But test-only code shouldn't be in production modules

## Categories of "Unused" Code

### Real Unused (Safe to Remove)
- Trait methods with 0 calls
- Abandoned feature implementations
- Orphaned helper functions
- Unused struct definitions

### False Positives (Keep)
- JSON DTOs (serde deserialization)
- Dynamic dispatch usage
- FFI boundaries
- Migration enums (SeaORM)

### Misleading (Fix Visibility)
- Private modules with public intent
- Internal APIs used by tests
- Cross-crate boundaries

## Lessons Learned

1. **Visibility matters more than you think** - Wrong visibility can hide massive amounts of used code
2. **Trait methods accumulate cruft** - Unused methods add maintenance burden
3. **Check errors, not just warnings** - Warnings can be misleading, errors are truth
4. **Remove aggressively** - If it's truly unused, delete it
5. **Document why** - If it looks unused but isn't, add comments

## Best Practices

1. Make modules public if they contain public API
2. Remove unused trait methods promptly
3. Use `#[cfg(test)]` for test-only code
4. Run `cargo check` after every major change
5. Check both errors AND warnings
6. Use tools systematically, not randomly