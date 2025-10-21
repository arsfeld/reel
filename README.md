<div align="center">
  <img src="logo.svg" alt="Reel Logo" width="128" height="128">
  
  # 🎬 Reel

  **A modern reactive media player for GNOME, built with Rust and Relm4 for performance and reliability.**

  [![CI](https://github.com/arsfeld/reel/actions/workflows/ci.yml/badge.svg)](https://github.com/arsfeld/reel/actions/workflows/ci.yml)
  [![codecov](https://codecov.io/gh/arsfeld/reel/branch/main/graph/badge.svg)](https://codecov.io/gh/arsfeld/reel)
  [![Rust](https://img.shields.io/badge/rust-1.89%2B-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org/)
  [![Relm4](https://img.shields.io/badge/Relm4-0.10-ff6b6b.svg?style=flat-square)](https://relm4.org/)
  [![License](https://img.shields.io/badge/license-GPLv3-green.svg?style=flat-square)](LICENSE)
  [![Nix Flakes](https://img.shields.io/badge/nix-flakes-5277C3.svg?style=flat-square&logo=nixos&logoColor=white)](https://nixos.wiki/wiki/Flakes)
  [![libadwaita](https://img.shields.io/badge/libadwaita-1.4-purple.svg?style=flat-square)](https://gnome.pages.gitlab.gnome.org/libadwaita/)
  
  <br/>
  
  [![Plex](https://img.shields.io/badge/Plex-✅_Supported-e5a00d.svg?style=for-the-badge&logo=plex&logoColor=white)](https://www.plex.tv/)
  [![Jellyfin](https://img.shields.io/badge/Jellyfin-✅_Supported-00A4DC.svg?style=for-the-badge&logo=jellyfin&logoColor=white)](https://jellyfin.org/)
  [![Local Files](https://img.shields.io/badge/Local_Files-🚧_Coming_Soon-grey.svg?style=for-the-badge)](https://github.com/arsfeld/reel)
</div>

> [!WARNING]
> **Relm4 Migration In Progress (~85% Complete)**: Reel is being migrated to a fully reactive Relm4 architecture. Core functionality is working but expect some UI polish issues and missing features as we complete the transition.

## What is Reel?

Reel is a native Linux media player that brings your Plex and Jellyfin libraries to the GNOME desktop. Written entirely in Rust with a reactive Relm4 UI, it leverages the language's performance and memory safety to deliver a fast, reliable media experience without the overhead of web technologies.

| Main Window | Show Details |
|:---:|:---:|
| ![Reel Screenshot - Movies Library](screenshots/main-window.png) | ![Reel Screenshot - Show Details](screenshots/show-details.png) |

| Video Player |
|:---:|
| ![Reel Screenshot - Video Player](screenshots/player.png) |

## ✨ Key Features

| Feature | Description |
|---------|-------------|
| **📴 Offline-First** | Full library metadata synced to local SQLite - browse your entire collection without internet (downloads coming soon) |
| **🦀 Pure Rust + Relm4** | Reactive UI with AsyncComponents, Factory patterns, and Worker components for background tasks |
| **🔌 Multi-Backend** | Simultaneous Plex and Jellyfin with connection monitoring, PIN profiles, and keyring credential storage |
| **💾 Intelligent Cache** | Database-driven chunk cache with progressive streaming, smart cleanup, and replay of watched content |
| **🔍 Full-Text Search** | Tantivy-powered instant search across all media with lazy-loaded cast/crew metadata |
| **🎥 Dual Players** | MPV (Linux default) and GStreamer (macOS/fallback) with skip intro/credits and progress sync |
| **⚙️ Live Config** | Hot-reload configuration without restart, hardware acceleration support |
| **🎨 Native GNOME** | Relm4/libadwaita with responsive design and seamless desktop integration |

## 🚀 Getting Started

This project uses Nix flakes to manage the development environment, ensuring all dependencies (including GStreamer plugins) are properly configured.

### 📋 Prerequisites

- Nix with flakes enabled
- Git

### 🔨 Building with Nix

```bash
# Clone the repository
git clone https://github.com/arsfeld/reel.git
cd reel

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

Download the latest release from the [Releases page](https://github.com/arsfeld/reel/releases/latest).

> [!WARNING]
> **AppImage Support Temporarily Removed**: AppImage builds have been removed due to packaging bugs. The application binary is fully functional and can be used as-is, provided you have the required dependencies installed (GTK4, libadwaita, GStreamer, and libmpv2 on Linux). Contributions to restore AppImage support are welcome!

> [!NOTE]
> **Pre-built packages are experimental**: These packages are automatically generated and may not be thoroughly tested. If you encounter issues, consider building from source using the Nix development environment.

### 📋 System Requirements

#### Minimum OS Versions
- **Ubuntu/Debian**: Ubuntu 24.04 LTS or newer (packages built against Ubuntu 24.04)
- **Fedora**: Fedora 40 or newer
- **Other distros**: Use the standalone binary or build from source for maximum compatibility

#### Required Libraries
| Library | Minimum Version | Notes |
|---------|-----------------|-------|
| **libadwaita** | 1.5 | UI toolkit |
| **Relm4** | 0.10.0 | Reactive UI framework |
| **GStreamer** | 1.20+ | Media framework with plugins-bad |
| **MPV** | libmpv2 0.29+ | Primary video player backend (Linux only) |
| **glibc** | 2.35+ | With 64-bit time_t support |
| **OpenSSL** | 3.0+ | TLS/SSL support |

> [!NOTE]
> **For older distributions**: If your system doesn't meet these requirements (e.g., Ubuntu 22.04, Fedora 39), you can use the standalone binary with manually installed dependencies, or build from source using the Nix development environment.

#### Standalone Binary
```bash
# Download the latest standalone binary (using getbin.io)
curl -fsSL https://getbin.io/arsfeld/reel?os=linux -o reel
chmod +x reel
./reel
# Note: Requires GTK4, libadwaita, and GStreamer to be installed
# MPV (libmpv2) is only required on Linux
```

#### Debian/Ubuntu (.deb)
```bash
# Download and install the latest .deb package (requires Ubuntu 24.04+)
# Get the latest release URL and download
curl -s https://api.github.com/repos/arsfeld/reel/releases/latest \
  | grep "browser_download_url.*amd64\.deb" \
  | cut -d '"' -f 4 \
  | xargs wget -O reel.deb
sudo dpkg -i reel.deb
sudo apt-get install -f  # Install dependencies if needed
```

#### Fedora/RHEL/openSUSE (.rpm)
```bash
# Download and install the latest .rpm package (requires Fedora 40+)
# Get the latest release URL and download
curl -s https://api.github.com/repos/arsfeld/reel/releases/latest \
  | grep "browser_download_url.*x86_64\.rpm" \
  | cut -d '"' -f 4 \
  | xargs wget -O reel.rpm
sudo dnf install ./reel.rpm
# or for older systems:
sudo rpm -i reel.rpm
```

### ❄️ Nix/NixOS

```bash
# Run directly with Nix flakes
nix run github:arsfeld/reel
```

### 📦 Flatpak

Flatpak support is available! You can build and install Reel using Flatpak for universal Linux compatibility.

#### Building Flatpak Locally

```bash
# Build and install using the included script
./scripts/build-flatpak.sh

# Or manually:
flatpak-builder --user --install --force-clean build-dir dev.arsfeld.Reel.json

# Run the application
flatpak run dev.arsfeld.Reel
```

#### Flathub Submission

Reel is being prepared for official Flathub distribution. See [docs/FLATHUB_SUBMISSION.md](docs/FLATHUB_SUBMISSION.md) for submission guidelines and requirements.

> [!NOTE]
> 📦 **Flathub Coming Soon** - Once submitted and approved, Reel will be available for easy installation via Flathub on all Linux distributions.

## 🏗️ Architecture

<details>
<summary><b>Click to see architecture diagram</b></summary>

Reel uses a pure Relm4 reactive architecture:

```
UI Layer (src/ui/)
├── AsyncComponents (Pages with data loading)
├── Factory Components (Dynamic collections)
├── Dialogs (Modal interactions)
└── Main Window & Sidebar (Navigation)
    ↓
Worker Components (src/workers/)
├── ConnectionMonitor (Health checks)
├── SyncWorker (Background sync)
├── SearchWorker (Search operations)
└── ImageLoader (Async image loading)
    ↓
Service Layer (src/services/)
├── MessageBrokers (Inter-component communication)
├── Commands (Structured async operations)
└── Core Services (Auth, Media, Sync, Source)
    ↓
Repository Layer (src/db/)
├── SeaORM Entities (Type-safe models)
└── Repository Pattern (CRUD operations)
    ↓
Backend Trait (src/backends/)
├── MediaBackend Interface
└── Implementations (Plex, Jellyfin, Local)
```

**Key Patterns:**
- **AsyncComponents**: Data-heavy pages with built-in loading states
- **Factory Pattern**: Efficient virtual scrolling for media grids
- **Worker Components**: Isolated background tasks (sync, image loading)
- **Command Pattern**: Type-safe async operations with proper lifecycle
- **Tracker Pattern**: Minimal re-renders through fine-grained change tracking
- **MessageBroker**: Replacing custom EventBus for component communication

</details>

The entire codebase leverages Rust's type system and ownership model to prevent common bugs at compile time, while the Relm4 reactive system ensures responsive UI updates without manual state management.

## 📊 Project Status

<p align="center">
  <a href="docs/journal.md">
    <img src="https://img.shields.io/badge/📖_Migration_Journal-docs%2Fjournal.md-purple?style=for-the-badge" alt="View Migration Progress"/>
  </a>
  <a href="https://github.com/MrLesk/Backlog.md">
    <img src="https://img.shields.io/badge/📋_Task_Management-Backlog.md-blue?style=for-the-badge" alt="Managed with Backlog.md"/>
  </a>
</p>

**Migration Progress**: ~85% complete

### ✅ What's Working

- **Relm4 UI Foundation** - ~85% complete migration to reactive component architecture
- **Multi-Backend Support** - Simultaneous Plex and Jellyfin with OAuth/credential auth
- **Media Playback** - MPV (Linux) and GStreamer (macOS/fallback) backends with OSD controls and keyboard shortcuts
- **Library Browsing** - Movies and TV shows with virtual scrolling and pagination
- **Continue Watching** - Progress tracking and resume functionality
- **Offline-First** - SQLite metadata cache for instant startup and offline browsing
- **File Cache** - Chunk-based progressive download with priority queue and fast seeking to any position
- **Source Management** - Add/remove/test/sync sources with automatic connection failover
- **GNOME Integration** - Native Relm4/libadwaita UI with proper NavigationSplitView

### ⚠️ Known Limitations

- **macOS**: Full support available (GStreamer backend), but pre-built binary coming soon - build from source using Nix for now
- GStreamer has subtitle color artifacts (use MPV player instead on Linux)
- Local files backend is 10% implemented (structure only)
- Some features require server-side support (e.g., Jellyfin chapter markers)

### 🔮 Coming Soon

- **Downloads** - Download media for offline playback on demand
- **Transcoding** - Server-side transcoding for incompatible formats
- **macOS Binary** - Pre-built macOS application bundle


## 🛠️ Tech Stack

- **Language**: Rust 2021 edition
- **UI Framework**: [Relm4](https://relm4.org/) + libadwaita
- **Database**: SQLite with [SeaORM](https://www.sea-ql.org/SeaORM/) and typed IDs
- **Async Runtime**: [Tokio](https://tokio.rs/) with MessageBroker for component communication
- **HTTP Client**: [Reqwest](https://github.com/seanmonstar/reqwest) with HTTP/2
- **Video Playback**: MPV (Linux default) via libmpv2, GStreamer (macOS default, fallback) via [gstreamer-rs](https://gitlab.freedesktop.org/gstreamer/gstreamer-rs)
- **Caching**: Three-tier (Memory LRU → SQLite → Backend API)
- **Serialization**: [Serde](https://serde.rs/)
- **Security**: System keyring via [keyring-rs](https://github.com/hwchen/keyring-rs)

## 🤝 Contributing

Contributions are welcome! This project uses [Backlog.md](https://github.com/MrLesk/Backlog.md) for task management. To see available tasks and contribute, use the `backlog` CLI tool after entering the development environment.

### Before Submitting a PR:
- Run `cargo fmt` to format your code
- Run `cargo clippy` to check for common issues
- Ensure all tests pass with `cargo test`
- Update documentation if needed

## 📄 License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

Built with excellent Rust crates and GNOME technologies:
- The [Relm4](https://relm4.org/) team for the reactive UI framework
- [GNOME](https://www.gnome.org/) for the beautiful desktop platform
- The Rust community for an amazing ecosystem of crates