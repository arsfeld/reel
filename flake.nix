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
          wrapGAppsHook4
          desktop-file-utils
          blueprint-compiler
        ];

        buildInputs = with pkgs; [
          # GTK and UI
          gtk4
          libadwaita
          libepoxy  # For OpenGL function loading
          
          # GStreamer and media
          gst_all_1.gstreamer
          gst_all_1.gst-plugins-base
          gst_all_1.gst-plugins-good
          gst_all_1.gst-plugins-bad
          gst_all_1.gst-plugins-ugly
          gst_all_1.gst-libav
          gst_all_1.gst-vaapi
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
          echo "Building AppImage..."
          
          # Ensure we have a release build
          cargo build --release
          
          VERSION=$(grep '^version' Cargo.toml | cut -d'"' -f2)
          
          # Clean up previous builds
          rm -rf AppDir
          rm -f *.AppImage
          
          # Create AppDir structure
          mkdir -p AppDir/usr/bin
          mkdir -p AppDir/usr/share/applications
          mkdir -p AppDir/usr/share/icons/hicolor/scalable/apps
          mkdir -p AppDir/usr/share/metainfo
          
          # Copy binary
          cp target/release/reel AppDir/usr/bin/
          chmod +x AppDir/usr/bin/reel
          
          # Copy desktop file
          cp data/dev.arsfeld.Reel.desktop AppDir/usr/share/applications/
          
          # Copy icon
          cp data/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg AppDir/usr/share/icons/hicolor/scalable/apps/
          
          # Create AppRun script
          cat > AppDir/AppRun << 'EOF'
          #!/bin/bash
          SELF=$(readlink -f "$0")
          HERE=''${SELF%/*}
          export PATH="''${HERE}/usr/bin:''${PATH}"
          export LD_LIBRARY_PATH="''${HERE}/usr/lib:''${LD_LIBRARY_PATH}"
          export XDG_DATA_DIRS="''${HERE}/usr/share:''${XDG_DATA_DIRS}"
          export GSETTINGS_SCHEMA_DIR="''${HERE}/usr/share/glib-2.0/schemas:''${GSETTINGS_SCHEMA_DIR}"
          exec "''${HERE}/usr/bin/reel" "$@"
          EOF
          chmod +x AppDir/AppRun
          
          # Download appimagetool if not available
          if ! command -v appimagetool &> /dev/null; then
            if [ ! -f appimagetool-x86_64.AppImage ]; then
              echo "Downloading appimagetool..."
              wget -q https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage
              chmod +x appimagetool-x86_64.AppImage
            fi
            APPIMAGETOOL="./appimagetool-x86_64.AppImage"
          else
            APPIMAGETOOL="appimagetool"
          fi
          
          # Create the AppImage (handle FUSE requirement)
          if ! ARCH=x86_64 "$APPIMAGETOOL" --no-appstream AppDir "Reel-$VERSION-x86_64.AppImage" 2>/dev/null; then
            echo "Trying with appimage-extract method..."
            "$APPIMAGETOOL" --appimage-extract >/dev/null 2>&1
            if [ -d squashfs-root ]; then
              ARCH=x86_64 ./squashfs-root/AppRun --no-appstream AppDir "Reel-$VERSION-x86_64.AppImage"
              rm -rf squashfs-root
            else
              echo "Note: AppImage creation requires FUSE. Install fuse or fuse3 package."
              echo "Alternatively, you can use the GitHub Actions workflow to build AppImages."
              exit 1
            fi
          fi
          
          if [ -f "Reel-$VERSION-x86_64.AppImage" ]; then
            echo "✓ AppImage built: Reel-$VERSION-x86_64.AppImage"
            echo ""
            echo "AppImage info:"
            file "Reel-$VERSION-x86_64.AppImage"
            ls -lh "Reel-$VERSION-x86_64.AppImage"
          else
            echo "✗ Failed to build AppImage"
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

        devTools = with pkgs; [
          # Development tools
          cargo-watch
          cargo-edit
          cargo-audit
          cargo-outdated
          cargo-nextest
          
          # Package building tools (will be installed via cargo)
          # cargo-deb and cargo-generate-rpm are installed as needed
          appimage-run
          
          # Database tools
          sqlx-cli
          
          # Code quality
          # rustfmt and clippy are provided by rustToolchain
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
          
          # Package testing tools
          dpkg
          rpm
          file
          wget
          fuse
        ];

      in
      {
        devShells.default = pkgs.mkShell {
          inherit buildInputs;
          nativeBuildInputs = rustBuildInputs;
          
          packages = devTools ++ [
            flatpakUpdateSources
            flatpakBuild
            flatpakBuildInstall
            flatpakRun
            flatpakLint
            formatCode
            clippyFix
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
            echo "Code quality commands:"
            echo "  format-code    - Format all Rust code with rustfmt"
            echo "  clippy-fix     - Run clippy and auto-fix issues"
            echo "  cargo fmt      - Format code (standard)"
            echo "  cargo clippy   - Run linter (standard)"
            echo ""
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
            
            # Force libmpv-sys to use system MPV
            export MPV_NO_PKG_CONFIG=0
            export DEP_MPV_VERSION_MAJOR=2
            export DEP_MPV_VERSION_MINOR=5
          '';

          # Environment variables
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;
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
            platforms = platforms.linux;
            mainProgram = "reel";
          };
        };
        
        packages.gnome-reel = self.packages.${system}.default;
      });
}