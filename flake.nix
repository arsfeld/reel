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

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };

        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkg-config
          meson
          ninja
          wrapGAppsHook4
          desktop-file-utils
          blueprint-compiler
        ];

        buildInputs = with pkgs; [
          # GTK and UI
          gtk4
          libadwaita
          
          # GStreamer and media
          gst_all_1.gstreamer
          gst_all_1.gst-plugins-base
          gst_all_1.gst-plugins-good
          gst_all_1.gst-plugins-bad
          gst_all_1.gst-plugins-ugly
          gst_all_1.gst-libav
          gst_all_1.gst-vaapi
          gst_all_1.gst-plugins-rs  # Includes gtk4paintablesink
          
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
        ];

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
          ${pkgs.flatpak-builder}/bin/flatpak-builder --user --install --force-clean build-dir dev.arsfeld.Reel.json
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
          rustfmt
          clippy
          pre-commit
          
          # Debugging
          gdb
          valgrind
          
          # Documentation
          mdbook
          
          # Flatpak tools
          appstream
          flatpak-builder
          pythonWithPkgs
        ];

      in
      {
        devShells.default = pkgs.mkShell {
          inherit buildInputs nativeBuildInputs;
          
          packages = devTools ++ [
            flatpakUpdateSources
            flatpakBuild
            flatpakBuildInstall
            flatpakLint
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
            echo "  cargo fmt      - Format code"
            echo "  cargo clippy   - Run linter"
            echo ""
            echo "Flatpak commands:"
            echo "  flatpak-update-sources - Update cargo-sources.json"
            echo "  flatpak-build         - Build the flatpak"
            echo "  flatpak-build-install - Build and install the flatpak"
            echo "  flatpak-lint          - Lint the flatpak manifest"
            echo ""
            
            # Initialize pre-commit hooks if not already done
            if [ ! -f .git/hooks/pre-commit ]; then
              echo "Installing pre-commit hooks..."
              pre-commit install
              echo "Pre-commit hooks installed!"
              echo ""
            fi
            
            # Set up GStreamer plugin paths - include core gstreamer plugins
            export GST_PLUGIN_SYSTEM_PATH_1_0="${pkgs.gst_all_1.gstreamer.out}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-base}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-good}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-bad}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-ugly}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-libav}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-rs}/lib/gstreamer-1.0"
            
            # Set up GTK schema paths
            export XDG_DATA_DIRS="${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk4}/share/gsettings-schemas/${pkgs.gtk4.name}:$XDG_DATA_DIRS"
            
            # Enable debug symbols for development
            export RUST_BACKTRACE=1
            
            # Set up pkg-config paths
            export PKG_CONFIG_PATH="${pkgs.lib.makeSearchPathOutput "dev" "lib/pkgconfig" buildInputs}:$PKG_CONFIG_PATH"
            
            # SQLx offline mode for development
            export SQLX_OFFLINE=true
          '';

          # Environment variables
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;
        };

        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "gnome-reel";
          version = "0.1.0";
          
          src = ./.;
          
          cargoLock = {
            lockFile = ./Cargo.lock;
          };
          
          inherit nativeBuildInputs buildInputs;
          
          # Skip tests during build (can be run separately)
          doCheck = false;
          
          meta = with pkgs.lib; {
            description = "A modern GTK frontend for Plex and other media servers";
            homepage = "https://github.com/arsfeld/gnome-reel";
            license = licenses.gpl3Plus;
            maintainers = [];
            platforms = platforms.linux;
          };
        };
      });
}