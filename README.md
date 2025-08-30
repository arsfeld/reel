<div align="center">
  <img src="logo.svg" alt="Reel Logo" width="128" height="128">
  
  # 🎬 Reel
  
  **A modern GTK4 media player for GNOME, built with Rust for performance and reliability.**
  
  [![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org/)
  [![GTK4](https://img.shields.io/badge/GTK-4.0-blue.svg?style=flat-square)](https://gtk.org/)
  [![License](https://img.shields.io/badge/license-GPLv3-green.svg?style=flat-square)](LICENSE)
  [![Nix Flakes](https://img.shields.io/badge/nix-flakes-5277C3.svg?style=flat-square&logo=nixos&logoColor=white)](https://nixos.wiki/wiki/Flakes)
  [![libadwaita](https://img.shields.io/badge/libadwaita-1.4-purple.svg?style=flat-square)](https://gnome.pages.gitlab.gnome.org/libadwaita/)
  
  <br/>
  
  [![Plex](https://img.shields.io/badge/Plex-✅_Supported-e5a00d.svg?style=for-the-badge&logo=plex&logoColor=white)](https://www.plex.tv/)
  [![Jellyfin](https://img.shields.io/badge/Jellyfin-✅_Supported-00A4DC.svg?style=for-the-badge&logo=jellyfin&logoColor=white)](https://jellyfin.org/)
  [![Local Files](https://img.shields.io/badge/Local_Files-🚧_Coming_Soon-grey.svg?style=for-the-badge)](https://github.com/arsfeld/gnome-reel)
</div>

> [!WARNING]
> **Early Development**: Reel is actively being developed. Expect rough edges, missing features, and breaking changes as we work toward a stable release.

## What is Reel?

Reel is a native, cross-platform media player that brings your Plex and Jellyfin libraries to the desktop with a premium, Netflix-like experience. Written entirely in Rust, it leverages the language's performance and memory safety to deliver a fast, reliable media experience without the overhead of web technologies. Currently focused on Linux/GNOME with GTK4, Reel is architected for multi-platform support with macOS development underway using both Swift-bridge and Cocoa (objc2) approaches.

| Main Window | Show Details |
|:---:|:---:|
| ![Reel Screenshot - Movies Library](screenshots/main-window.png) | ![Reel Screenshot - Show Details](screenshots/show-details.png) |

| Video Player |
|:---:|
| ![Reel Screenshot - Video Player](screenshots/player.png) |

## ✨ Key Features

| Feature | Description |
|---------|-------------|
| **🦀 Pure Rust** | Fast, memory-safe, and concurrent by design with modern architecture |
| **🔌 Multi-Backend** | Simultaneous Plex and Jellyfin connections, local files in development |
| **💾 Offline-First** | SeaORM/SQLite database with instant UI loading and background sync |
| **🎨 Native GTK4** | Beautiful libadwaita interface following GNOME HIG |
| **⚡ Reactive UI** | Event-driven ViewModels with observable properties for responsive updates |
| **🎥 Dual Players** | MPV (default, recommended) and GStreamer backends for maximum compatibility |
| **🔐 Secure Auth** | OAuth for Plex, secure credential storage in system keyring |
| **📊 Smart Caching** | Three-tier caching strategy (Memory LRU → SQLite → Backend API) |

## 🚀 Getting Started

This project uses Nix flakes to manage the development environment, ensuring all dependencies (including GStreamer plugins and GTK schemas) are properly configured.

### 📋 Prerequisites

- Nix with flakes enabled
- Git

### 🔨 Building with Nix

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

### 💻 Development Commands

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

### 📥 Download Pre-built Packages

Download the latest release from the [Releases page](https://github.com/arsfeld/gnome-reel/releases/latest).

> [!WARNING]
> **Pre-built packages are experimental**: These packages are automatically generated and may not be thoroughly tested. If you encounter issues, consider building from source using the Nix development environment.

#### AppImage (Universal - Recommended)
```bash
# Download the AppImage
wget https://github.com/arsfeld/gnome-reel/releases/latest/download/reel-v0.3.0-x86_64.AppImage
chmod +x reel-*.AppImage
./reel-*.AppImage
```

#### Debian/Ubuntu (.deb)
```bash
# Download and install the .deb package
wget https://github.com/arsfeld/gnome-reel/releases/latest/download/reel-v0.3.0-amd64.deb
sudo dpkg -i reel-*.deb
sudo apt-get install -f  # Install dependencies if needed
```

#### Fedora/RHEL/openSUSE (.rpm)
```bash
# Download and install the .rpm package
wget https://github.com/arsfeld/gnome-reel/releases/latest/download/reel-v0.3.0-x86_64.rpm
sudo dnf install ./reel-*.rpm
# or for older systems:
sudo rpm -i reel-*.rpm
```

### ❄️ Nix/NixOS

```bash
# Run directly with Nix flakes
nix run github:arsfeld/gnome-reel
```

### 📦 Flatpak

> [!NOTE]
> 🚧 **Coming Soon** - Flatpak packaging is planned to make Reel available across all Linux distributions.

## 🏗️ Architecture

<details>
<summary><b>Click to see architecture diagram</b></summary>

Reel implements a modern reactive architecture with clean separation of concerns:

```
UI Layer (GTK4/libadwaita with Blueprint templates)
    ↓
ViewModels (Reactive properties with change notifications)
    ↓
Event System (Tokio broadcast-based event bus)
    ↓
Service Layer (DataService, SyncManager, SourceCoordinator)
    ↓
Repository Pattern (Type-safe SeaORM repositories)
    ↓
Database Layer (SQLite with migrations and caching)
    ↓
Backend Trait (Generic MediaBackend interface)
    ↓
Implementations (Plex, Jellyfin, Local Files)
```

**Key Architectural Patterns:**
- **Reactive ViewModels**: Observable properties that automatically update UI
- **Event-Driven Updates**: System-wide event broadcasting for data changes
- **Repository Pattern**: Clean data access layer with SeaORM
- **Three-Tier Caching**: Memory (LRU) → Database (SQLite) → Backend API
- **Platform Abstraction**: Frontend trait enables multi-platform support

</details>

The entire codebase leverages Rust's type system and ownership model to prevent common bugs at compile time, while async/await with Tokio enables efficient handling of network requests and media operations. The application is currently 75% through a migration to a fully reactive architecture.

## 📊 Project Status

<p align="center">
  <a href="TASKS.md">
    <img src="https://img.shields.io/badge/📋_View_Full_Roadmap-TASKS.md-blue?style=for-the-badge" alt="View Roadmap"/>
  </a>
</p>

### ✅ Completed Features
- **Authentication & Server Management**
  - Plex OAuth authentication with PIN-based flow
  - Jellyfin username/password authentication
  - Automatic server discovery and connection
  - Multi-backend architecture supporting Plex and Jellyfin simultaneously
  - Secure credential storage in system keyring
  - Sources page for backend management with exciting UI
  - AuthProvider/Source separation for flexible authentication

- **Media Browsing & Playback**
  - Complete movie and TV show libraries with responsive grid views
  - Cinematic detail pages with backdrop images and rich metadata
  - **MPV player backend (default)** - Superior performance, no subtitle issues
  - GStreamer player backend (secondary) - Available but has subtitle color artifacts
  - Immersive player with auto-hiding controls and fullscreen support
  - Audio/subtitle track selection with on-the-fly switching
  - Watch status tracking with playback progress indicators
  - Resume from last position across sessions
  - Continue watching and recently added sections on homepage

- **Performance & Architecture**
  - **Reactive ViewModels** - Observable properties with automatic UI updates
  - **Event-driven architecture** - System-wide event bus for data changes
  - **SeaORM database layer** - Type-safe queries with migrations
  - **Three-tier caching** - Memory (LRU) → SQLite → Backend API
  - Multi-level image caching with request coalescing
  - HTTP/2 connection pooling for faster API calls
  - Lazy loading with viewport-based rendering
  - Offline-first with instant UI from cache
  - Async/await throughout with Tokio runtime

- **User Experience**
  - Homepage with dynamic content sections from all backends
  - Library filtering (watched/unwatched/all) and sorting (title/year/rating/date)
  - Library visibility management per backend
  - Modern Blueprint-based UI following GNOME HIG
  - Smooth transitions and consistent loading states
  - Fullscreen playback - F11, double-click, cursor auto-hiding
  - Advanced player controls - Keyboard shortcuts, draggable window, time display modes
  - Backend switcher for seamless source selection

### 🔧 In Development
- **Architecture Migration (75% Complete)**
  - Migrating to fully reactive architecture with ViewModels
  - Completing event system integration (12/27 event types done)
  - Integrating remaining UI pages with ViewModels (4 of 6 pages remaining)
  - Adding transaction support to sync operations

- **Known Issues to Fix**
  - Homepage sections randomly replace each other when multiple backends are enabled
  - Horizontal scrolling on homepage doesn't load images
  - GStreamer subtitle color artifacts (use MPV player instead)
  - Main Window hybrid status system causing race conditions

- **Search & Filtering**
  - Search UI implementation (Jellyfin backend ready, Plex needs work)
  - Advanced filtering (genre, year, rating, resolution)
  - Collections and playlists support

- **Media Information**
  - Cast and crew information display UI (data available from Jellyfin)
  - Media badges (4K, HDR, Dolby Vision, etc.)
  - Enhanced metadata display with ratings from multiple sources

- **Additional Features**
  - Local file library scanning (10% implemented)
  - Music and photo library support
  - Settings management migration to GSettings
  - Offline download and playback functionality
  - Metadata provider integration (TMDB, TVDB)
  - Skip intro/credits improvements
  - macOS native UI (Swift-bridge and Cocoa implementations in progress)


## 🛠️ Tech Stack

- **Language**: Rust 2024 edition with async/await
- **UI Framework**: GTK4 + libadwaita via [gtk-rs](https://gtk-rs.org/)
- **Async Runtime**: [Tokio](https://tokio.rs/) with broadcast channels for events
- **Database**: SQLite with [SeaORM](https://www.sea-ql.org/SeaORM/) for type-safe queries
- **HTTP Client**: [Reqwest](https://github.com/seanmonstar/reqwest) with connection pooling
- **Video Playback**: MPV (default) via libmpv2, GStreamer (secondary) via [gstreamer-rs](https://gitlab.freedesktop.org/gstreamer/gstreamer-rs)
- **Caching**: LRU memory cache + SQLite persistent cache
- **Serialization**: [Serde](https://serde.rs/) for JSON/TOML
- **Security**: [Keyring](https://github.com/hwchen/keyring-rs) for credential storage

## 🤝 Contributing

Contributions are welcome! Since this is an early-stage Rust project, please check [TASKS.md](TASKS.md) for areas needing work.

### Before Submitting a PR:
- Run `cargo fmt` to format your code
- Run `cargo clippy` to check for common issues
- Ensure all tests pass with `cargo test`
- Update documentation if needed

## 📄 License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

Built with excellent Rust crates and GNOME technologies:
- The [gtk-rs](https://gtk-rs.org/) team for exceptional Rust bindings
- [GNOME](https://www.gnome.org/) for the beautiful desktop platform
- The Rust community for an amazing ecosystem of crates