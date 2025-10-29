# GitHub Copilot Instructions for Reel

## Project Overview

Reel is a native, cross-platform media player application written in Rust that brings your Plex and Jellyfin libraries to the desktop with a premium, Netflix-like experience. The project uses:

- **Language**: Rust 2021 edition with strict quality standards
- **UI Framework**: Relm4 (reactive component-based) with libadwaita for GNOME integration
- **Architecture**: Offline-first with SQLite cache, multi-backend support (Plex, Jellyfin, Local files)
- **Build System**: Cargo with Nix flakes for reproducible development environment
- **Platform**: Linux (primary), macOS (in development)

## Development Environment Setup

**CRITICAL**: This project uses Nix flakes for dependency management. All development commands must be run within the Nix development shell:

```bash
# Enter the development environment (REQUIRED FIRST)
nix develop

# Then proceed with cargo commands
cargo build
cargo run
```

The Nix environment provides:
- Correct Rust toolchain version
- GStreamer plugins and development libraries
- MPV/libmpv2 development libraries (Linux)
- GTK4 and libadwaita development files
- All system dependencies properly configured

## Build Commands

```bash
# Check compilation without building
cargo check

# Build debug version
cargo build

# Build release version
cargo build --release

# Run the application
cargo run

# Run with specific features (e.g., macOS without MPV)
cargo run --no-default-features --features gstreamer
```

**Important**: Always use `cargo check` or `cargo build` to verify compilation. The last line of output shows the actual number of errors/warnings.

## Code Quality

### Formatting
```bash
# Format all Rust code (REQUIRED before committing)
cargo fmt

# Check formatting without modifying files
cargo fmt -- --check
```

Pre-commit hooks automatically run `cargo fmt`, so code should always be formatted.

### Linting
```bash
# Run Clippy lints (REQUIRED before committing)
cargo clippy

# Run Clippy on all targets
cargo clippy --all-targets

# Run Clippy with specific features
cargo clippy --no-default-features --features gstreamer
```

**Standards**: Fix all Clippy warnings. Do not use `#[allow]` attributes without strong justification.

### Testing
```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture

# Run tests for specific package
cargo test -p reel
```

## Architecture Guidelines

### Module Structure

- `src/main.rs` - Entry point and application initialization
- `src/app/` - Application core with Relm4 AsyncComponent
- `src/ui/` - All UI components (pages, dialogs, factories, shared)
  - Use AsyncComponents for data-heavy pages
  - Use Factory pattern for collections (media grids, lists)
  - Use Worker components for background tasks
- `src/services/` - Service layer with brokers, commands, and core services
- `src/backends/` - Media server integrations (all implement `MediaBackend` trait)
- `src/db/` - Database layer with SeaORM entities and repositories
- `src/player/` - Media playback (MPV for Linux, GStreamer for macOS)
- `src/workers/` - Background worker components
- `src/models/` - Data models and type-safe identifiers

### Key Design Patterns

1. **Relm4 Reactive Architecture**
   - Use AsyncComponents for pages with data loading
   - Use `#[tracker::track]` for efficient change tracking
   - Use Factory pattern (FactoryVecDeque) for dynamic collections
   - Use Worker components for background tasks
   - Use MessageBroker for inter-component communication
   - Use Command pattern for structured async operations

2. **Backend Abstraction**
   - All media sources implement the `MediaBackend` trait
   - Never hardcode backend-specific logic outside backend implementations
   - Use the repository pattern for data access

3. **Offline-First Architecture**
   - Always load from SQLite cache first
   - Update UI immediately with cached data
   - Sync from backend in background
   - Handle offline scenarios gracefully

4. **Three-Tier Caching**
   - Memory Cache (LRU, 1000 items)
   - Database Cache (SQLite with SeaORM)
   - Backend API (source-specific)

### Code Style

- **No backward compatibility code**: Tackle the root cause, not symptoms
- **No fallbacks**: They introduce confusion and errors
- **Minimal comments**: Code should be self-documenting; add comments only for complex logic
- **Type safety**: Use newtype patterns for IDs and domain concepts
- **Error handling**: Use `Result` types, provide context with error chains
- **Async/await**: Use Tokio runtime, avoid blocking operations

## Platform-Specific Notes

### Linux
- Default features: Both MPV and GStreamer backends
- MPV is the primary/preferred player backend
- Full hardware acceleration support

### macOS
- Use `--no-default-features --features gstreamer` (MPV has OpenGL issues)
- GStreamer is the only supported backend
- Some features may be limited compared to Linux

## Testing Guidelines

- Write tests for new functionality
- Follow existing test patterns in the repository
- Tests should be deterministic and not depend on external services
- Use mocks/stubs for backend integrations
- Test both success and error paths

## Common Tasks

### Adding a New UI Component
1. Create component file in appropriate `src/ui/` subdirectory
2. Implement as SimpleComponent, Component, or AsyncComponent based on needs
3. Define component message enum for user interactions
4. Use `#[tracker::track]` on model for efficient updates
5. Implement `view()` method with Relm4 macros
6. Add to parent component's factory or direct inclusion

### Adding a New Backend Feature
1. Update `MediaBackend` trait in `src/backends/traits.rs`
2. Implement for each backend (Plex, Jellyfin, Local)
3. Update database schema if needed (add migration)
4. Update repository layer for data access
5. Add service layer methods if needed
6. Update UI components to use new feature

### Adding Database Entities
1. Create migration in `src/db/migrations/`
2. Define SeaORM entity in `src/db/entities/`
3. Create/update repository in `src/db/repository/`
4. Update service layer to use new entity
5. Run migration: `sea-orm-cli migrate up`

## Important Rules

1. **Don't mention AI tools in regular commit messages** - Use descriptive, technical commit messages (exception: setup/configuration commits related to AI tool integration)
2. **Do not add backward compatibility layers** - Fix the root cause
3. **Do not use fallbacks** - They add confusion; use proper error handling
4. **Always format code** - Run `cargo fmt` before committing
5. **Fix Clippy warnings** - Don't ignore or suppress them without justification
6. **Work in Nix shell** - All commands require the Nix development environment
7. **Test your changes** - Run relevant tests after code changes
8. **Use existing patterns** - Follow the established architecture and code style

## Documentation

- Update RELEASE.md when adding user-facing features
- Update README.md for significant feature additions
- Keep CLAUDE.md in sync with major architectural changes
- Add inline documentation for complex algorithms only

## Dependencies

Key dependencies (see Cargo.toml for full list):
- **relm4**: 0.10 - Reactive UI framework
- **libadwaita**: 0.8 - GNOME UI components
- **tokio**: 1.47 - Async runtime
- **sea-orm**: Database ORM
- **reqwest**: HTTP client
- **libmpv2**: MPV player backend (Linux)
- **gstreamer**: GStreamer player backend (all platforms)

When adding dependencies:
- Check if existing dependencies can solve the problem
- Prefer well-maintained crates with active development
- Avoid duplicating functionality
- Update Cargo.toml with appropriate features

## Troubleshooting

### Build Issues
- Ensure you're in `nix develop` shell
- Check that all system dependencies are available in Nix environment
- Try `cargo clean` and rebuild
- Check Nix flake.nix for environment configuration

### Test Failures
- Run tests with `-- --nocapture` to see detailed output
- Check if tests depend on specific system state
- Verify database migrations are current

### Player Issues
- Linux: MPV is default, GStreamer is fallback
- macOS: Must use GStreamer (build with `--no-default-features --features gstreamer`)
- Check GStreamer plugins are loaded (configured in Nix environment)

## CI/CD

The project uses GitHub Actions:
- `ci.yml`: Runs on all PRs (build, test, clippy, fmt)
- `release.yml`: Creates release packages (deb, rpm, binary)
- All checks must pass before merging

When making changes:
- Ensure CI passes before requesting review
- Fix any formatting or clippy issues
- Add tests for new functionality

## Getting Help

- Check existing code for patterns and examples
- Review CLAUDE.md for detailed architecture documentation
- See README.md for user-facing features and setup
- Check docs/ directory for additional documentation
