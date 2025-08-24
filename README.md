<div align="center">
  <img src="logo.svg" alt="Reel Logo" width="128" height="128">
  
  # 🎬 Reel
  
  **A modern GTK4 media player for GNOME, built with Rust for performance and reliability.**
</div>

> **⚠️ Early Development**: Reel is actively being developed. Expect rough edges, missing features, and breaking changes as we work toward a stable release.

## What is Reel?

Reel is a native Linux media player that brings your Plex (and soon Jellyfin) libraries to the GNOME desktop. Written entirely in Rust, it leverages the language's performance and memory safety to deliver a fast, reliable media experience without the overhead of web technologies.

![Reel Screenshot - Movies Library](screenshots/main-window.png)
![Reel Screenshot - Show Details](screenshots/show-details.png)

### ✨ Key Features

- **🦀 Pure Rust Implementation**: Fast, memory-safe, and concurrent by design
- **🔌 Multi-Backend Architecture**: Currently supports Plex, with Jellyfin and local files planned
- **💾 Offline-First Design**: SQLite caching keeps your library browsable even offline
- **🎨 Native GTK4/libadwaita**: Seamlessly integrates with modern GNOME desktops
- **⚡ Async Everything**: Built on Tokio for responsive, non-blocking operations
- **🎥 GStreamer Playback**: Hardware-accelerated video with comprehensive codec support

## 🚀 Getting Started

This project uses Nix flakes to manage the development environment, ensuring all dependencies (including GStreamer plugins and GTK schemas) are properly configured.

### Prerequisites

- Nix with flakes enabled
- Git

### Building with Nix

```bash
# Clone the repository
git clone https://github.com/arsfeld/gnome-reel.git
cd gnome-reel

# Enter the Nix development shell
nix develop

# Build the Rust project
cargo build

# Run the application
cargo run
```

### Development Commands

Inside the Nix shell:

```bash
# Format Rust code
cargo fmt

# Run Clippy lints
cargo clippy

# Run test suite
cargo test

# Build optimized release binary
cargo build --release
```

## 📦 Installation

### Nix/NixOS

```bash
# Run directly with Nix flakes
nix run github:arsfeld/gnome-reel
```

### Flatpak

🚧 **Coming Soon** - Flatpak packaging is planned to make Reel available across all Linux distributions.

## 🏗️ Architecture

Reel follows Rust best practices with a clean separation of concerns:

```
UI Layer (GTK4/Blueprint templates)
    ↓
Application State (Arc<RwLock> shared state)
    ↓
Service Layer (Tokio async services)
    ↓
Backend Trait (Generic MediaBackend interface)
    ↓
Implementations (Plex, Jellyfin, Local)
```

The entire codebase leverages Rust's type system and ownership model to prevent common bugs at compile time, while async/await enables efficient handling of network requests and media operations.

## 📊 Project Status

See [TASKS.md](TASKS.md) for the detailed development roadmap.

### ✅ Completed Features
- **Authentication & Server Management**
  - Plex OAuth authentication with PIN-based flow
  - Automatic server discovery and connection
  - Multi-backend architecture supporting Plex, Jellyfin (planned), and local files (planned)
  - Persistent authentication and server preferences

- **Media Browsing & Playback**
  - Complete movie and TV show libraries with grid views
  - Cinematic detail pages with backdrop images and metadata
  - **MPV player backend (default)** - Superior performance with no subtitle issues
  - GStreamer player backend (secondary) - Available but has subtitle color artifacts
  - Immersive player with auto-hiding controls
  - Audio/subtitle track selection
  - Watch status tracking and progress indicators
  - Continue watching and recently added sections

- **Performance & Architecture**
  - Multi-level image caching (memory + disk) with request coalescing
  - HTTP/2 connection pooling for faster API calls
  - Lazy loading with viewport-based rendering
  - SQLite-based offline cache for instant startup
  - Backend-agnostic UI architecture for extensibility
  - Async/await throughout with Tokio runtime

- **User Experience**
  - Homepage with dynamic content sections
  - Library filtering (watched/unwatched) and sorting
  - Library visibility management
  - Modern Blueprint-based UI with GNOME HIG compliance
  - Smooth transitions and loading states
  - **Fullscreen playback support** - F11, double-click, and cursor auto-hiding
  - **Advanced player controls** - Keyboard shortcuts, window dragging, time display modes

### 🔧 In Development
- Playback position syncing to server
- Advanced filtering (genre, year, rating)
- Search functionality
- Music and photo library support
- Settings management with GSettings


## 🛠️ Tech Stack

- **Language**: Rust 2021 edition
- **UI Framework**: GTK4 + libadwaita via [gtk-rs](https://gtk-rs.org/)
- **Async Runtime**: [Tokio](https://tokio.rs/)
- **Database**: SQLite with [SQLx](https://github.com/launchbadge/sqlx)
- **HTTP Client**: [Reqwest](https://github.com/seanmonstar/reqwest)
- **Video Playback**: MPV (default) via libmpv2, GStreamer (secondary) via [gstreamer-rs](https://gitlab.freedesktop.org/gstreamer/gstreamer-rs)
- **Serialization**: [Serde](https://serde.rs/)

## 🤝 Contributing

Contributions are welcome! Since this is an early-stage Rust project, please check [TASKS.md](TASKS.md) for areas needing work. The codebase follows standard Rust conventions - run `cargo fmt` and `cargo clippy` before submitting PRs.

## 📄 License

[License details to be determined]

## 🙏 Acknowledgments

Built with excellent Rust crates and GNOME technologies:
- The [gtk-rs](https://gtk-rs.org/) team for exceptional Rust bindings
- [GNOME](https://www.gnome.org/) for the beautiful desktop platform
- The Rust community for an amazing ecosystem of crates