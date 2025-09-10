<div align="center">
  <img src="logo.svg" alt="Reel Logo" width="128" height="128">
  
  # 🎬 Reel
  
  **A modern GTK4 media player for GNOME, built with Rust for performance and reliability.**
  
  [![CI](https://github.com/arsfeld/reel/actions/workflows/ci.yml/badge.svg)](https://github.com/arsfeld/reel/actions/workflows/ci.yml)
  [![Rust](https://img.shields.io/badge/rust-1.89%2B-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org/)
  [![GTK4](https://img.shields.io/badge/GTK-4.14%2B-blue.svg?style=flat-square)](https://gtk.org/)
  [![License](https://img.shields.io/badge/license-GPLv3-green.svg?style=flat-square)](LICENSE)
  [![Nix Flakes](https://img.shields.io/badge/nix-flakes-5277C3.svg?style=flat-square&logo=nixos&logoColor=white)](https://nixos.wiki/wiki/Flakes)
  [![libadwaita](https://img.shields.io/badge/libadwaita-1.4-purple.svg?style=flat-square)](https://gnome.pages.gitlab.gnome.org/libadwaita/)
  
  <br/>
  
  [![Plex](https://img.shields.io/badge/Plex-✅_Supported-e5a00d.svg?style=for-the-badge&logo=plex&logoColor=white)](https://www.plex.tv/)
  [![Jellyfin](https://img.shields.io/badge/Jellyfin-✅_Supported-00A4DC.svg?style=for-the-badge&logo=jellyfin&logoColor=white)](https://jellyfin.org/)
  [![Local Files](https://img.shields.io/badge/Local_Files-🚧_Coming_Soon-grey.svg?style=for-the-badge)](https://github.com/arsfeld/reel)
</div>

> [!WARNING]
> **Early Development**: Reel is actively being developed. Expect rough edges, missing features, and breaking changes as we work toward a stable release.

## What is Reel?

Reel is a native Linux media player that brings your Plex and Jellyfin libraries to the GNOME desktop. Written entirely in Rust, it leverages the language's performance and memory safety to deliver a fast, reliable media experience without the overhead of web technologies.

| Main Window | Show Details |
|:---:|:---:|
| ![Reel Screenshot - Movies Library](screenshots/main-window.png) | ![Reel Screenshot - Show Details](screenshots/show-details.png) |

| Video Player |
|:---:|
| ![Reel Screenshot - Video Player](screenshots/player.png) |

## ✨ Key Features

| Feature | Description |
|---------|-------------|
| **🦀 Pure Rust** | Fast, memory-safe, and concurrent by design |
| **🔌 Multi-Backend** | Supports Plex and Jellyfin, with local files planned |
| **💾 Offline-First** | SQLite caching keeps your library browsable even offline |
| **🎨 Native GTK4** | Seamlessly integrates with modern GNOME desktops |
| **⚡ Async Everything** | Built on Tokio for responsive, non-blocking operations |
| **🎥 Dual Players** | MPV (default) and GStreamer backends for maximum compatibility |

## 🚀 Getting Started

This project uses Nix flakes to manage the development environment, ensuring all dependencies (including GStreamer plugins and GTK schemas) are properly configured.

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
> **Pre-built packages are experimental**: These packages are automatically generated and may not be thoroughly tested. If you encounter issues, consider building from source using the Nix development environment.

### 📋 System Requirements

#### Minimum OS Versions
- **Ubuntu/Debian**: Ubuntu 24.04 LTS or newer (packages built against Ubuntu 24.04)
- **Fedora**: Fedora 40 or newer
- **Other distros**: Use AppImage or Flatpak for maximum compatibility

#### Required Libraries
| Library | Minimum Version | Notes |
|---------|-----------------|-------|
| **libadwaita** | 1.5 | UI toolkit |
| **GTK4** | 4.13.5 | GUI framework |
| **GStreamer** | 1.20+ | Media framework with plugins-bad |
| **MPV** | libmpv2 0.29+ | Primary video player backend |
| **glibc** | 2.35+ | With 64-bit time_t support |
| **OpenSSL** | 3.0+ | TLS/SSL support |

> [!NOTE]
> **For older distributions**: If your system doesn't meet these requirements (e.g., Ubuntu 22.04, Fedora 39), please use the **AppImage** or **Flatpak** versions which bundle their own dependencies.

#### AppImage (Universal - Recommended)
```bash
# Download the AppImage
wget https://github.com/arsfeld/reel/releases/latest/download/reel-v0.4.0-x86_64.AppImage
chmod +x reel-*.AppImage
./reel-*.AppImage
```

#### Debian/Ubuntu (.deb)
```bash
# Download and install the .deb package (requires Ubuntu 24.04+)
wget https://github.com/arsfeld/reel/releases/latest/download/reel-v0.4.0-amd64.deb
sudo dpkg -i reel-*.deb
sudo apt-get install -f  # Install dependencies if needed
```

#### Fedora/RHEL/openSUSE (.rpm)
```bash
# Download and install the .rpm package (requires Fedora 40+)
wget https://github.com/arsfeld/reel/releases/latest/download/reel-v0.4.0-x86_64.rpm
sudo dnf install ./reel-*.rpm
# or for older systems:
sudo rpm -i reel-*.rpm
```

### ❄️ Nix/NixOS

```bash
# Run directly with Nix flakes
nix run github:arsfeld/reel
```

### 📦 Flatpak

> [!NOTE]
> 🚧 **Coming Soon** - Flatpak packaging is planned to make Reel available across all Linux distributions.

## 🏗️ Architecture

<details>
<summary><b>Click to see architecture diagram</b></summary>

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

</details>

The entire codebase leverages Rust's type system and ownership model to prevent common bugs at compile time, while async/await enables efficient handling of network requests and media operations.

## 📊 Project Status

<p align="center">
  <a href="TASKS.md">
    <img src="https://img.shields.io/badge/📋_Detailed_Roadmap-TASKS.md-blue?style=for-the-badge" alt="View Full Roadmap"/>
  </a>
</p>

### ✅ What's Working

- **Multi-Backend Support** - Connect to Plex and Jellyfin simultaneously
- **Media Playback** - MPV (recommended) and GStreamer player backends
- **Library Browsing** - Movies and TV shows with filtering and sorting
- **Offline-First** - SQLite cache for instant startup and offline browsing
- **Modern Architecture** - Reactive ViewModels with SeaORM database layer
- **GNOME Integration** - Native GTK4/libadwaita UI following HIG

### 🚧 Major Roadmap Items

- **Search** - Backend implementations exist, UI needed
- **Cast & Crew** - Display UI for existing backend data
- **Local Files** - Scan and play local media libraries
- **Advanced Filters** - Genre, year, rating, resolution
- **Settings** - Migrate to GSettings for GNOME compliance
- **Offline Playback** - Download and sync for offline viewing

### ⚠️ Known Limitations

- GStreamer has subtitle rendering issues (use MPV)
- Search not yet available in UI
- Local files backend is mostly unimplemented
- Some features require server-side support (e.g., Jellyfin chapter markers)


## 🛠️ Tech Stack

- **Language**: Rust 2021 edition
- **UI Framework**: GTK4 + libadwaita via [gtk-rs](https://gtk-rs.org/)
- **Database**: SQLite with [SeaORM](https://www.sea-ql.org/SeaORM/) (reactive architecture)
- **Async Runtime**: [Tokio](https://tokio.rs/) with channels for event broadcasting
- **HTTP Client**: [Reqwest](https://github.com/seanmonstar/reqwest) with HTTP/2
- **Video Playback**: MPV (default) via libmpv2, GStreamer (fallback) via [gstreamer-rs](https://gitlab.freedesktop.org/gstreamer/gstreamer-rs)
- **Caching**: Three-tier (Memory LRU → SQLite → Backend API)
- **Serialization**: [Serde](https://serde.rs/)
- **Security**: System keyring via [keyring-rs](https://github.com/hwchen/keyring-rs)

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