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
          
          # Localization
          gettext
          
          # Image processing
          librsvg
        ];

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
          
          # Debugging
          gdb
          valgrind
          
          # Documentation
          mdbook
        ];

      in
      {
        devShells.default = pkgs.mkShell {
          inherit buildInputs nativeBuildInputs;
          
          packages = devTools;

          shellHook = ''
            echo "Plex GTK Development Environment"
            echo "================================"
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
            
            # Set up GStreamer plugin paths
            export GST_PLUGIN_SYSTEM_PATH_1_0="${pkgs.gst_all_1.gst-plugins-base}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-good}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-bad}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-ugly}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-libav}/lib/gstreamer-1.0"
            
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
          pname = "plex-gtk";
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
            homepage = "https://github.com/arsfeld/plex-gtk";
            license = licenses.gpl3Plus;
            maintainers = [];
            platforms = platforms.linux;
          };
        };
      });
}