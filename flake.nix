{
  description = "Reel - A modern, native media player for the GNOME desktop";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable."1.89.0".default.override {
          extensions = [ "rust-src" "rust-analyzer" "rustfmt" "clippy" ];
        };

        # Build inputs needed for compiling the Rust project
        rustBuildInputs = with pkgs; [
          rustToolchain
          pkg-config
          desktop-file-utils
          blueprint-compiler
          meson
          ninja
        ] ++ lib.optionals pkgs.stdenv.isLinux [
          wrapGAppsHook4  # Linux-only for wrapping GTK apps
        ];

        # Platform-specific packages
        linuxOnlyPackages = with pkgs; lib.optionals pkgs.stdenv.isLinux [
          gst_all_1.gst-vaapi  # VA-API is Linux-only
        ];

        darwinOnlyPackages = with pkgs; lib.optionals pkgs.stdenv.isDarwin [
          darwin.apple_sdk.frameworks.CoreFoundation
          darwin.apple_sdk.frameworks.CoreServices
          darwin.apple_sdk.frameworks.Security
          darwin.apple_sdk.frameworks.SystemConfiguration
          darwin.apple_sdk.frameworks.AppKit
        ];

        buildInputs = with pkgs; [
          # GTK and UI
          gtk4  # This should be 4.14+ from nixpkgs unstable
          libadwaita
          libepoxy  # For OpenGL function loading
          
          # Additional UI dependencies for Slint
          fontconfig
          freetype
          
          # GStreamer and media
          gst_all_1.gstreamer
          gst_all_1.gst-plugins-base
          gst_all_1.gst-plugins-good
          gst_all_1.gst-plugins-bad
          gst_all_1.gst-plugins-ugly
          gst_all_1.gst-libav
          gst_all_1.gst-plugins-rs  # Includes gtk4paintablesink
          
          # MPV for alternative player backend
          mpv
          
          # System libraries
          glib
          cairo
          pango
          gdk-pixbuf
          graphene
          
          # Database
          sqlite
          
          # Networking and crypto
          openssl
          curl
          
          # Keyring support
          libsecret
          dbus
          dbus.dev
          
          # Localization
          gettext
          
          # Image processing
          librsvg
        ] ++ lib.optionals pkgs.stdenv.isLinux [
          # Wayland support for Slint (Linux-only)
          wayland
          wayland-protocols
          libxkbcommon
          
          # OpenGL (Linux-specific)
          libGL
          libglvnd
        ] ++ linuxOnlyPackages ++ darwinOnlyPackages;

        pythonWithPkgs = pkgs.python3.withPackages (ps: with ps; [
          aiohttp
          toml
          tomlkit
        ]);

        flatpakUpdateSources = pkgs.writeShellScriptBin "flatpak-update-sources" ''
          echo "Updating cargo-sources.json..."
          ${pythonWithPkgs}/bin/python3 flatpak-cargo-generator.py Cargo.lock -o cargo-sources.json
          echo "cargo-sources.json updated!"
        '';

        flatpakBuild = pkgs.writeShellScriptBin "flatpak-build" ''
          if [ ! -f cargo-sources.json ]; then
            echo "cargo-sources.json not found, generating it..."
            ${pythonWithPkgs}/bin/python3 flatpak-cargo-generator.py Cargo.lock -o cargo-sources.json
          fi
          echo "Building flatpak..."
          ${pkgs.flatpak-builder}/bin/flatpak-builder --force-clean build-dir dev.arsfeld.Reel.json
        '';

        flatpakBuildInstall = pkgs.writeShellScriptBin "flatpak-build-install" ''
          if [ ! -f cargo-sources.json ]; then
            echo "cargo-sources.json not found, generating it..."
            ${pythonWithPkgs}/bin/python3 flatpak-cargo-generator.py Cargo.lock -o cargo-sources.json
          fi
          
          echo "Building and installing flatpak..."
          ${pkgs.flatpak-builder}/bin/flatpak-builder --user --install --force-clean --disable-rofiles-fuse build-dir dev.arsfeld.Reel.json
        '';

        flatpakRun = pkgs.writeShellScriptBin "flatpak-run" ''
          echo "Running Reel flatpak..."
          flatpak run dev.arsfeld.Reel
        '';

        flatpakLint = pkgs.writeShellScriptBin "flatpak-lint" ''
          echo "Linting flatpak manifest..."
          if command -v flatpak-builder-lint &> /dev/null; then
            flatpak-builder-lint manifest dev.arsfeld.Reel.json
          else
            echo "flatpak-builder-lint not found. You can install it via:"
            echo "  flatpak install flathub org.flatpak.Builder"
            echo "  flatpak run --command=flatpak-builder-lint org.flatpak.Builder manifest dev.arsfeld.Reel.json"
          fi
        '';

        formatCode = pkgs.writeShellScriptBin "format-code" ''
          echo "Formatting Rust code..."
          ${rustToolchain}/bin/cargo fmt
          echo "Code formatting complete!"
        '';

        clippyFix = pkgs.writeShellScriptBin "clippy-fix" ''
          echo "Running clippy with auto-fix..."
          ${rustToolchain}/bin/cargo clippy --fix --allow-dirty --allow-staged
          echo "Clippy fixes applied!"
        '';

        # Package building scripts
        buildDeb = pkgs.writeShellScriptBin "build-deb" ''
          echo "Building Debian package..."
          
          # Install cargo-deb if not available
          if ! command -v cargo-deb &> /dev/null; then
            echo "Installing cargo-deb..."
            cargo install cargo-deb --locked
          fi
          
          # Ensure we have a release build
          cargo build --release
          
          # Build the deb package
          cargo deb --no-build
          
          DEB_FILE=$(find target/debian -name "*.deb" -type f | head -n1)
          if [ -n "$DEB_FILE" ]; then
            echo "✓ Debian package built: $DEB_FILE"
            echo ""
            echo "Package info:"
            dpkg-deb -I "$DEB_FILE"
            echo ""
            echo "Package contents:"
            dpkg-deb -c "$DEB_FILE" | head -20
            echo "..."
          else
            echo "✗ Failed to build Debian package"
            exit 1
          fi
        '';

        buildRpm = pkgs.writeShellScriptBin "build-rpm" ''
          echo "Building RPM package..."
          
          # Install cargo-generate-rpm if not available
          if ! command -v cargo-generate-rpm &> /dev/null; then
            echo "Installing cargo-generate-rpm..."
            cargo install cargo-generate-rpm --locked
          fi
          
          # Ensure we have a release build
          cargo build --release
          
          # Build the RPM package
          cargo generate-rpm
          
          RPM_FILE=$(find target/generate-rpm -name "*.rpm" -type f | head -n1)
          if [ -n "$RPM_FILE" ]; then
            echo "✓ RPM package built: $RPM_FILE"
            echo ""
            echo "Package info:"
            rpm -qip "$RPM_FILE"
            echo ""
            echo "Package contents:"
            rpm -qlp "$RPM_FILE" | head -20
            echo "..."
          else
            echo "✗ Failed to build RPM package"
            exit 1
          fi
        '';

        buildAppImage = pkgs.writeShellScriptBin "build-appimage" ''
          echo "Building AppImage using Docker (Ubuntu environment)..."
          
          # Build the Docker image that matches the GitHub Actions environment
          ${pkgs.docker}/bin/docker build -t reel-appimage-builder -f- . <<'DOCKERFILE'
          FROM ubuntu:latest
          
          # Install system dependencies (matching .github/workflows/release.yml)
          RUN apt-get update && apt-get install -y \
              libgtk-4-dev \
              libadwaita-1-dev \
              libgstreamer1.0-dev \
              libgstreamer-plugins-base1.0-dev \
              libgstreamer-plugins-bad1.0-dev \
              gstreamer1.0-plugins-base \
              gstreamer1.0-plugins-good \
              gstreamer1.0-plugins-bad \
              gstreamer1.0-plugins-ugly \
              gstreamer1.0-libav \
              libmpv-dev \
              libsqlite3-dev \
              pkg-config \
              libssl-dev \
              libdbus-1-dev \
              blueprint-compiler \
              rpm \
              file \
              curl \
              build-essential \
              libfuse2 \
              desktop-file-utils \
              zsync \
              wget \
              ca-certificates \
              patchelf
          
          # Install Rust
          RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
          ENV PATH="/root/.cargo/bin:''${PATH}"
          
          WORKDIR /workspace
          DOCKERFILE
          
          # Run the build inside the Docker container (builds release binary + AppImage)
          ${pkgs.docker}/bin/docker run --rm -v "$(pwd):/workspace" reel-appimage-builder bash -c '
            echo "=== Building release binary with Ubuntu dependencies ==="
            cargo build --release
            strip target/release/reel
            echo "✓ Release binary built successfully"
            echo ""
            
            # Now build the AppImage
            ./build-appimage.sh
          '
          
          APPIMAGE_FILE=$(find . -maxdepth 1 -name "reel-*.AppImage" -type f | head -n1)
          if [ -n "$APPIMAGE_FILE" ]; then
            echo "✓ AppImage built successfully: $APPIMAGE_FILE"
            echo "  Size: $(du -h "$APPIMAGE_FILE" | cut -f1)"
            echo "  File type: $(file "$APPIMAGE_FILE")"
          else
            echo "✗ No AppImage found in current directory"
            exit 1
          fi
        '';

        buildAllPackages = pkgs.writeShellScriptBin "build-all-packages" ''
          echo "Building all package formats..."
          echo "=============================="
          echo ""
          
          # Build release binary first
          echo "Building release binary..."
          cargo build --release
          echo ""
          
          # Build each package type
          echo "1. Building Debian package..."
          echo "------------------------------"
          build-deb
          echo ""
          
          echo "2. Building RPM package..."
          echo "------------------------------"
          build-rpm
          echo ""
          
          echo "3. Building AppImage..."
          echo "------------------------------"
          build-appimage
          echo ""
          
          echo "=============================="
          echo "All packages built successfully!"
          echo ""
          echo "Package files:"
          find target/debian -name "*.deb" -type f 2>/dev/null | xargs -I {} echo "  - Debian: {}"
          find target/generate-rpm -name "*.rpm" -type f 2>/dev/null | xargs -I {} echo "  - RPM: {}"
          find . -maxdepth 1 -name "*.AppImage" -type f 2>/dev/null | xargs -I {} echo "  - AppImage: {}"
        '';

        # Meson build commands
        mesonSetup = pkgs.writeShellScriptBin "meson-setup" ''
          echo "Setting up Meson build directory..."
          meson setup builddir --prefix=$HOME/.local
          echo "Meson build directory configured!"
        '';

        mesonBuild = pkgs.writeShellScriptBin "meson-build" ''
          echo "Building with Meson..."
          if [ ! -d builddir ]; then
            echo "Build directory not found. Running meson setup first..."
            meson setup builddir --prefix=$HOME/.local
          fi
          meson compile -C builddir
          echo "Build complete!"
        '';

        mesonInstall = pkgs.writeShellScriptBin "meson-install" ''
          echo "Installing with Meson..."
          if [ ! -d builddir ]; then
            echo "Build directory not found. Running meson setup first..."
            meson setup builddir --prefix=$HOME/.local
          fi
          meson install -C builddir
          echo "Installation complete!"
        '';

        mesonTest = pkgs.writeShellScriptBin "meson-test" ''
          echo "Running Meson tests..."
          if [ ! -d builddir ]; then
            echo "Build directory not found. Running meson setup first..."
            meson setup builddir --prefix=$HOME/.local
          fi
          meson test -C builddir
        '';

        mesonClean = pkgs.writeShellScriptBin "meson-clean" ''
          echo "Cleaning Meson build directory..."
          if [ -d builddir ]; then
            rm -rf builddir
            echo "Build directory removed."
          else
            echo "Build directory not found."
          fi
        '';

        mesonDist = pkgs.writeShellScriptBin "meson-dist" ''
          echo "Creating distribution tarball with Meson..."
          if [ ! -d builddir ]; then
            echo "Build directory not found. Running meson setup first..."
            meson setup builddir --prefix=$HOME/.local
          fi
          meson dist -C builddir --no-tests
          echo "Distribution tarball created in builddir/meson-dist/"
        '';

        devTools = with pkgs; [
          # Development tools
          cargo-watch
          cargo-edit
          cargo-audit
          cargo-outdated
          cargo-nextest
          
          # Database tools
          sqlx-cli
          
          # Code quality
          # rustfmt and clippy are provided by rustToolchain
          pre-commit
          
          # Documentation
          mdbook
          
          # Flatpak tools (Linux-only)
          pythonWithPkgs
          
          # Common package testing tools
          file
          wget
          
          # AppImage building tools
          python3
          python3Packages.pip
          
        ] ++ lib.optionals pkgs.stdenv.isLinux [
          # Build optimization tools (Linux-only)
          mold         # Fast linker
          clang        # Compiler for mold
          # Linux-specific tools
          appimage-run
          gdb
          valgrind
          appstream
          flatpak-builder
          dpkg
          rpm
          fuse
          docker
        ] ++ lib.optionals pkgs.stdenv.isDarwin [
          # macOS-specific debugging tools
          lldb
        ];

      in
      {
        devShells.default = pkgs.mkShell {
          inherit buildInputs;
          nativeBuildInputs = rustBuildInputs;
          
          packages = devTools ++ [
            formatCode
            clippyFix
            mesonSetup
            mesonBuild
            mesonInstall
            mesonTest
            mesonClean
            mesonDist
          ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
            flatpakUpdateSources
            flatpakBuild
            flatpakBuildInstall
            flatpakRun
            flatpakLint
            buildDeb
            buildRpm
            buildAppImage
            buildAllPackages
          ];

          shellHook = ''
            echo "Gnome Reel Development Environment"
            echo "=================================="
            echo ""
            echo "Rust toolchain: $(rustc --version)"
            echo "Cargo: $(cargo --version)"
            echo ""
            echo "Available commands:"
            echo "  cargo build    - Build the project"
            echo "  cargo run      - Run the application"
            echo "  cargo test     - Run tests"
            echo "  cargo watch    - Watch for changes and rebuild"
            echo ""
            echo "Meson build commands:"
            echo "  meson-setup    - Setup Meson build directory"
            echo "  meson-build    - Build with Meson"
            echo "  meson-install  - Install with Meson"
            echo "  meson-test     - Run Meson tests"
            echo "  meson-clean    - Clean Meson build directory"
            echo "  meson-dist     - Create distribution tarball"
            echo ""
            echo "Code quality commands:"
            echo "  format-code    - Format all Rust code with rustfmt"
            echo "  clippy-fix     - Run clippy and auto-fix issues"
            echo "  cargo fmt      - Format code (standard)"
            echo "  cargo clippy   - Run linter (standard)"
            echo ""
            ${pkgs.lib.optionalString pkgs.stdenv.isLinux ''
              echo "Package building commands:"
              echo "  build-deb          - Build Debian package (.deb)"
              echo "  build-rpm          - Build RPM package (.rpm)"
              echo "  build-appimage     - Build AppImage"
              echo "  build-all-packages - Build all package formats"
              echo ""
              echo "Flatpak commands:"
              echo "  flatpak-update-sources - Update cargo-sources.json"
              echo "  flatpak-build         - Build the flatpak"
              echo "  flatpak-build-install - Build and install the flatpak"
              echo "  flatpak-run           - Run the installed flatpak"
              echo "  flatpak-lint          - Lint the flatpak manifest"
              echo ""
            ''}
            
            # Initialize pre-commit hooks if not already done
            if [ ! -f .git/hooks/pre-commit ]; then
              echo "Installing pre-commit hooks..."
              pre-commit install
              echo "Pre-commit hooks installed!"
              echo ""
            fi
            
            # Set up GStreamer plugin paths - include core gstreamer plugins
            GST_PATHS="${pkgs.gst_all_1.gstreamer.out}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-base}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-good}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-bad}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-ugly}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-libav}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-rs}/lib/gstreamer-1.0"
            ${pkgs.lib.optionalString pkgs.stdenv.isLinux ''
              GST_PATHS="$GST_PATHS:${pkgs.gst_all_1.gst-vaapi}/lib/gstreamer-1.0"
            ''}
            export GST_PLUGIN_SYSTEM_PATH_1_0="$GST_PATHS"
            
            # Set up GTK schema paths
            export XDG_DATA_DIRS="${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk4}/share/gsettings-schemas/${pkgs.gtk4.name}:$XDG_DATA_DIRS"
            
            # Enable debug symbols for development
            export RUST_BACKTRACE=1
            
            # Set up pkg-config paths
            export PKG_CONFIG_PATH="${pkgs.lib.makeSearchPathOutput "dev" "lib/pkgconfig" buildInputs}:$PKG_CONFIG_PATH"
            
            # SQLx offline mode for development
            export SQLX_OFFLINE=true
            
            # Force libmpv-sys to use system MPV
            export MPV_NO_PKG_CONFIG=0
            export DEP_MPV_VERSION_MAJOR=2
            export DEP_MPV_VERSION_MINOR=5
            
            # Fix gettext-sys on macOS
            ${pkgs.lib.optionalString pkgs.stdenv.isDarwin ''
              export GETTEXT_DIR="${pkgs.gettext}"
              export GETTEXT_LIB_DIR="${pkgs.gettext}/lib"
              export GETTEXT_INCLUDE_DIR="${pkgs.gettext}/include"
              export GETTEXT_BIN_DIR="${pkgs.gettext}/bin"
              export GETTEXT_SYSTEM=1
            ''}
            
            # Configure mold linker for faster builds (Linux-only)
            ${pkgs.lib.optionalString pkgs.stdenv.isLinux ''
              export RUSTFLAGS="-C link-arg=-fuse-ld=mold"
              export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="clang"
            ''}
            
            # Parallel compilation optimizations
            export CARGO_BUILD_JOBS=$(nproc)
            export CARGO_NET_GIT_FETCH_WITH_CLI=true
            
            # Enable incremental compilation for faster development builds
            export CARGO_INCREMENTAL=1
            
            # Memory optimization for large projects
            export CARGO_TARGET_DIR="$PWD/target"
            
          '';

          # Environment variables
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;
          
          # Fix gettext-sys build on macOS
          GETTEXT_DIR = if pkgs.stdenv.isDarwin then "${pkgs.gettext}" else "";
          GETTEXT_LIB_DIR = if pkgs.stdenv.isDarwin then "${pkgs.gettext}/lib" else "";
          GETTEXT_INCLUDE_DIR = if pkgs.stdenv.isDarwin then "${pkgs.gettext}/include" else "";
          GETTEXT_BIN_DIR = if pkgs.stdenv.isDarwin then "${pkgs.gettext}/bin" else "";
          GETTEXT_SYSTEM = if pkgs.stdenv.isDarwin then "1" else "";
        };

        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "gnome-reel";
          version = "0.3.0";
          
          src = ./.;
          
          cargoLock = {
            lockFile = ./Cargo.lock;
          };
          
          nativeBuildInputs = rustBuildInputs;
          
          inherit buildInputs;
          
          # Skip tests during build (can be run separately)
          doCheck = false;
          
          # Create a wrapper script that points to the actual binary
          postInstall = ''
            # The cargo build produces 'reel' binary, create symlink for 'gnome-reel'
            if [ -f $out/bin/reel ]; then
              ln -s $out/bin/reel $out/bin/gnome-reel
            fi
          '';
          
          meta = with pkgs.lib; {
            description = "A modern GTK frontend for Plex and other media servers";
            homepage = "https://github.com/arsfeld/gnome-reel";
            license = licenses.gpl3Plus;
            maintainers = [];
            platforms = platforms.linux ++ platforms.darwin;
            mainProgram = "reel";
          };
        };
        
        packages.gnome-reel = self.packages.${system}.default;
      });
}