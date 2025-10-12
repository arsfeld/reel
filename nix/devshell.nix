{
  pkgs,
  rustToolchain,
  buildInputs,
  rustBuildInputs,
  whitesurTheme,
  whitesurIconTheme,
  linuxOnlyPackages,
  darwinOnlyPackages,
  gdkPixbufWithSvg ? null,
  ...
}:
let
  pythonWithPkgs = pkgs.python3.withPackages (ps:
    with ps; [
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

  helpCommand = pkgs.writeShellScriptBin "help" ''
    echo "Reel Development Environment"
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
    ''}
    ${pkgs.lib.optionalString pkgs.stdenv.isLinux ''
      echo "Flatpak commands:"
      echo "  flatpak-update-sources - Update cargo-sources.json for flatpak"
      echo "  flatpak-build          - Build the flatpak"
      echo "  flatpak-build-install  - Build and install the flatpak"
      echo "  flatpak-run            - Run the installed flatpak"
      echo "  flatpak-lint           - Lint the flatpak manifest"
      echo ""
    ''}
    ${pkgs.lib.optionalString pkgs.stdenv.isDarwin ''
      echo "macOS commands:"
      echo "  build-macos-bundle - Build macOS app bundle"
      echo ""
    ''}
    echo "Release commands:"
    echo "  make-release        - Create a new release"
    echo "  release-and-push    - Create and push release"
    echo ""
    echo "Type 'help' to see this message again."
  '';

  formatCode = pkgs.writeShellScriptBin "format-code" ''
    echo "Formatting Rust code..."
    cargo fmt --all
    echo "Code formatting complete!"
  '';

  clippyFix = pkgs.writeShellScriptBin "clippy-fix" ''
    echo "Running clippy with auto-fix..."
    cargo clippy --fix --allow-dirty --allow-staged --all-features
    echo "Clippy fixes applied!"
  '';

  # Package building commands (Linux-only)
  buildDeb = pkgs.writeShellScriptBin "build-deb" ''
    echo "Building Debian package..."
    echo "=============================="

    # Build release binary
    echo "Building release binary..."
    cargo build --release

    # Create debian package structure
    PKG_NAME="reel"
    VERSION=$(grep "^version" Cargo.toml | head -1 | cut -d'"' -f2)
    DEB_DIR="target/debian/$PKG_NAME-$VERSION"

    echo "Creating package structure for $PKG_NAME version $VERSION..."
    rm -rf "$DEB_DIR"
    mkdir -p "$DEB_DIR/DEBIAN"
    mkdir -p "$DEB_DIR/usr/bin"
    mkdir -p "$DEB_DIR/usr/share/applications"
    mkdir -p "$DEB_DIR/usr/share/icons/hicolor/scalable/apps"

    # Copy binary
    cp target/release/reel "$DEB_DIR/usr/bin/reel"
    chmod 755 "$DEB_DIR/usr/bin/reel"

    # Copy desktop file
    if [ -f data/dev.arsfeld.Reel.desktop ]; then
      cp data/dev.arsfeld.Reel.desktop "$DEB_DIR/usr/share/applications/"
    fi

    # Copy icon
    if [ -f data/icons/dev.arsfeld.Reel.svg ]; then
      cp data/icons/dev.arsfeld.Reel.svg "$DEB_DIR/usr/share/icons/hicolor/scalable/apps/"
    fi

    # Create control file
    cat > "$DEB_DIR/DEBIAN/control" << EOF
    Package: $PKG_NAME
    Version: $VERSION
    Section: video
    Priority: optional
    Architecture: amd64
    Depends: libgtk-4-1, libadwaita-1-0, libmpv2, gstreamer1.0-plugins-base, gstreamer1.0-plugins-good, gstreamer1.0-plugins-bad, gstreamer1.0-plugins-ugly, gstreamer1.0-libav
    Maintainer: Reel Development Team
    Description: A modern GTK frontend for Plex and other media servers
     Reel is a native media player application that brings your Plex,
     Jellyfin and other media libraries to the GNOME desktop with a
     premium, Netflix-like experience.
    EOF

    # Build the package
    echo "Building .deb package..."
    dpkg-deb --build "$DEB_DIR" "target/debian/$PKG_NAME-$VERSION.deb"

    echo ""
    echo "✓ Debian package built successfully!"
    echo "  Package: target/debian/$PKG_NAME-$VERSION.deb"
    echo ""
    echo "To install: sudo dpkg -i target/debian/$PKG_NAME-$VERSION.deb"
  '';

  buildRpm = pkgs.writeShellScriptBin "build-rpm" ''
    echo "Building RPM package..."
    echo "=============================="

    # Build release binary
    echo "Building release binary..."
    cargo build --release

    PKG_NAME="reel"
    VERSION=$(grep "^version" Cargo.toml | head -1 | cut -d'"' -f2)

    # Create RPM build structure
    RPM_DIR="$HOME/rpmbuild"
    mkdir -p "$RPM_DIR"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}

    # Create tarball of the source
    echo "Creating source tarball..."
    tar czf "$RPM_DIR/SOURCES/$PKG_NAME-$VERSION.tar.gz" --exclude=target --exclude=.git .

    # Create spec file
    cat > "$RPM_DIR/SPECS/$PKG_NAME.spec" << EOF
    Name:           $PKG_NAME
    Version:        $VERSION
    Release:        1%{?dist}
    Summary:        A modern GTK frontend for Plex and other media servers
    License:        GPL-3.0+
    URL:            https://github.com/arsfeld/reel
    Source0:        %{name}-%{version}.tar.gz

    BuildRequires:  rust cargo gtk4-devel libadwaita-devel
    Requires:       gtk4 libadwaita mpv-libs gstreamer1-plugins-base gstreamer1-plugins-good gstreamer1-plugins-bad-free gstreamer1-plugins-ugly-free

    %description
    Reel is a native media player application that brings your Plex,
    Jellyfin and other media libraries to the GNOME desktop with a
    premium, Netflix-like experience.

    %prep
    %setup -q

    %build
    cargo build --release

    %install
    rm -rf %{buildroot}
    mkdir -p %{buildroot}%{_bindir}
    mkdir -p %{buildroot}%{_datadir}/applications
    mkdir -p %{buildroot}%{_datadir}/icons/hicolor/scalable/apps

    install -m 755 target/release/reel %{buildroot}%{_bindir}/reel
    install -m 644 data/dev.arsfeld.Reel.desktop %{buildroot}%{_datadir}/applications/
    install -m 644 data/icons/dev.arsfeld.Reel.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/

    %files
    %{_bindir}/reel
    %{_datadir}/applications/dev.arsfeld.Reel.desktop
    %{_datadir}/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg

    %changelog
    * $(date +"%a %b %d %Y") Reel Development Team
    - Initial RPM release
    EOF

    # Build the RPM
    echo "Building RPM package..."
    rpmbuild -ba "$RPM_DIR/SPECS/$PKG_NAME.spec"

    # Copy the built RPM to target directory
    mkdir -p target/rpm
    cp "$RPM_DIR/RPMS/x86_64/$PKG_NAME-$VERSION-1"*.rpm target/rpm/ 2>/dev/null || true

    echo ""
    echo "✓ RPM package built successfully!"
    echo "  Package: target/rpm/$PKG_NAME-$VERSION-1.*.rpm"
    echo ""
    echo "To install: sudo rpm -i target/rpm/$PKG_NAME-$VERSION-1.*.rpm"
  '';

  buildAppImage = pkgs.writeShellScriptBin "build-appimage" ''
    echo "Building AppImage..."
    echo "=============================="

    # Check for AppImage tools
    if ! command -v appimagetool &> /dev/null; then
      echo "Downloading appimagetool..."
      wget -q "https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage" -O /tmp/appimagetool
      chmod +x /tmp/appimagetool
      APPIMAGETOOL="/tmp/appimagetool"
    else
      APPIMAGETOOL="appimagetool"
    fi

    # Build release binary
    echo "Building release binary..."
    cargo build --release

    PKG_NAME="reel"
    VERSION=$(grep "^version" Cargo.toml | head -1 | cut -d'"' -f2)
    APP_DIR="target/appimage/$PKG_NAME.AppDir"

    echo "Creating AppImage structure..."
    rm -rf "$APP_DIR"
    mkdir -p "$APP_DIR/usr/bin"
    mkdir -p "$APP_DIR/usr/share/applications"
    mkdir -p "$APP_DIR/usr/share/icons/hicolor/scalable/apps"
    mkdir -p "$APP_DIR/usr/lib"

    # Copy binary
    cp target/release/reel "$APP_DIR/usr/bin/reel"
    chmod 755 "$APP_DIR/usr/bin/reel"

    # Copy desktop file
    if [ -f data/dev.arsfeld.Reel.desktop ]; then
      cp data/dev.arsfeld.Reel.desktop "$APP_DIR/usr/share/applications/"
      # Also copy to root for AppImage
      cp data/dev.arsfeld.Reel.desktop "$APP_DIR/"
    fi

    # Copy icon
    if [ -f data/icons/dev.arsfeld.Reel.svg ]; then
      cp data/icons/dev.arsfeld.Reel.svg "$APP_DIR/usr/share/icons/hicolor/scalable/apps/"
      # Convert SVG to PNG for AppImage icon
      if command -v rsvg-convert &> /dev/null; then
        rsvg-convert -w 256 -h 256 data/icons/dev.arsfeld.Reel.svg -o "$APP_DIR/dev.arsfeld.Reel.png"
      else
        # Fallback: just copy the SVG
        cp data/icons/dev.arsfeld.Reel.svg "$APP_DIR/"
      fi
    fi

    # Create AppRun script
    cat > "$APP_DIR/AppRun" << 'EOF'
    #!/bin/bash
    SELF=$(readlink -f "$0")
    HERE="$(dirname "$SELF")"
    export PATH="$HERE/usr/bin:$PATH"
    export LD_LIBRARY_PATH="$HERE/usr/lib:$LD_LIBRARY_PATH"
    export XDG_DATA_DIRS="$HERE/usr/share:$XDG_DATA_DIRS"
    exec "$HERE/usr/bin/reel" "$@"
    EOF
    chmod 755 "$APP_DIR/AppRun"

    # Copy required libraries (simplified - in production would use linuxdeploy)
    echo "Note: For a production AppImage, use linuxdeploy to bundle all dependencies"

    # Build AppImage
    echo "Building AppImage package..."
    ARCH=x86_64 "$APPIMAGETOOL" "$APP_DIR" "target/appimage/$PKG_NAME-$VERSION-x86_64.AppImage"

    echo ""
    echo "✓ AppImage built successfully!"
    echo "  Package: target/appimage/$PKG_NAME-$VERSION-x86_64.AppImage"
    echo ""
    echo "To run: ./target/appimage/$PKG_NAME-$VERSION-x86_64.AppImage"
  '';

  buildAllPackages = pkgs.writeShellScriptBin "build-all-packages" ''
    echo "Building all package formats..."
    echo "================================"
    echo ""

    # Build Debian package
    echo "1/3: Building Debian package..."
    build-deb
    echo ""

    # Build RPM package
    echo "2/3: Building RPM package..."
    build-rpm
    echo ""

    # Build AppImage
    echo "3/3: Building AppImage..."
    build-appimage
    echo ""

    echo "================================"
    echo "✓ All packages built successfully!"
    echo ""
    echo "Packages created:"
    ls -la target/debian/*.deb 2>/dev/null | tail -1 || echo "  - Debian package build failed"
    ls -la target/rpm/*.rpm 2>/dev/null | tail -1 || echo "  - RPM package build failed"
    ls -la target/appimage/*.AppImage 2>/dev/null | tail -1 || echo "  - AppImage build failed"
  '';

  # macOS-specific bundle tools
  macOSBundleTools = with pkgs;
    lib.optionals pkgs.stdenv.isDarwin [
      cargo-bundle
      macdylibbundler
      librsvg # For rsvg-convert
    ];

  # macOS bundle building
  buildMacosBundle = pkgs.writeShellScriptBin "build-macos-bundle" ''
    echo "Building macOS app bundle..."
    echo "=============================="

    # Set environment variables for bundling
    export WHITESUR_GTK_THEME="${whitesurTheme}"
    export WHITESUR_ICON_THEME="${whitesurIconTheme}"
    ${pkgs.lib.optionalString pkgs.stdenv.isDarwin ''
      export SDL2="${pkgs.SDL2}"
    ''}

    # Ensure scripts directory exists
    if [ ! -f scripts/build-macos-bundle.sh ]; then
      echo "Error: scripts/build-macos-bundle.sh not found"
      echo "Please ensure you're in the project root directory"
      exit 1
    fi

    # All required tools are provided by Nix, except iconutil which is system-provided
    echo "Using Nix-provided tools:"
    echo "  - cargo-bundle: $(which cargo-bundle)"
    echo "  - rsvg-convert: $(which rsvg-convert)"
    echo "  - dylibbundler: $(which dylibbundler)"

    # Check for system-provided iconutil (can't be installed via Nix)
    if ! command -v iconutil &> /dev/null; then
      echo "Note: iconutil not found (part of Xcode Command Line Tools)"
      echo "Will use rsvg-convert + manual icns creation as fallback"
    fi

    # Run the build script with all tools available
    bash scripts/build-macos-bundle.sh
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

  # Release automation scripts
  makeRelease = pkgs.writeShellScriptBin "make-release" ''
    bash scripts/make-release.sh "$@"
  '';

  releaseAndPush = pkgs.writeShellScriptBin "release-and-push" ''
    bash scripts/release-and-push.sh "$@"
  '';

  devTools = with pkgs;
    [
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
    ]
    ++ lib.optionals pkgs.stdenv.isLinux [
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

      # High-performance linker for faster builds
      mold
    ]
    ++ lib.optionals pkgs.stdenv.isDarwin [
      # macOS-specific debugging tools
      lldb
    ] ++ macOSBundleTools;

in
pkgs.mkShell {
  inherit buildInputs;
  nativeBuildInputs = rustBuildInputs;

  packages =
    devTools
    ++ [
      helpCommand
      formatCode
      clippyFix
      mesonSetup
      mesonBuild
      mesonInstall
      mesonTest
      mesonClean
      mesonDist
      makeRelease
      releaseAndPush
    ]
    ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
      flatpakUpdateSources
      flatpakBuild
      flatpakBuildInstall
      flatpakRun
      flatpakLint
      buildDeb
      buildRpm
      buildAppImage
      buildAllPackages
    ]
    ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
      buildMacosBundle
    ];

  shellHook = ''
    echo "Hello!"

    # Initialize pre-commit hooks if not already done
    if [ ! -f .git/hooks/pre-commit ]; then
      echo "Installing pre-commit hooks..."
      pre-commit install
      echo "Pre-commit hooks installed!"
      echo ""
    fi

    # Set RUSTFLAGS - temporarily disable warnings as errors for development
    # export RUSTFLAGS="-D warnings"

    # Set library paths for mold linker on Linux
    # Use target-specific RUSTFLAGS to ensure they merge with .cargo/config.toml
    ${pkgs.lib.optionalString pkgs.stdenv.isLinux ''
      # Build library search paths for all buildInputs
      LIB_PATHS="${pkgs.lib.makeLibraryPath buildInputs}"
      LINK_ARGS=""
      # Split colon-separated paths and create individual -L flags for each
      for lib in $(echo "$LIB_PATHS" | tr ':' ' '); do
        LINK_ARGS="$LINK_ARGS -C link-arg=-L$lib"
      done

      # Set target-specific RUSTFLAGS which merge with config file rustflags
      export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="$LINK_ARGS"
      export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS="$LINK_ARGS"
    ''}

    # Set up GStreamer plugin paths - include core gstreamer plugins
    GST_PATHS="${pkgs.gst_all_1.gstreamer.out}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-base}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-good}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-bad}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-ugly}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-libav}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-rs}/lib/gstreamer-1.0"
    ${pkgs.lib.optionalString pkgs.stdenv.isLinux ''
      GST_PATHS="$GST_PATHS:${pkgs.gst_all_1.gst-vaapi}/lib/gstreamer-1.0"
    ''}
    export GST_PLUGIN_SYSTEM_PATH_1_0="$GST_PATHS"

    # Set up GTK schema paths
    export XDG_DATA_DIRS="${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk4}/share/gsettings-schemas/${pkgs.gtk4.name}:$XDG_DATA_DIRS"

    # Set up WhiteSur GTK theme and icon themes for macOS
    ${pkgs.lib.optionalString pkgs.stdenv.isDarwin ''
      export GTK_THEME=WhiteSur-Dark
      export XDG_DATA_DIRS="${whitesurTheme}/share:${whitesurIconTheme}/share:${pkgs.adwaita-icon-theme}/share:${pkgs.hicolor-icon-theme}/share:$XDG_DATA_DIRS"
      # Set icon theme search path for GTK
      export GTK_PATH="${pkgs.gtk4}/lib/gtk-4.0:$GTK_PATH"
      # Explicitly set the icon theme name to WhiteSur-dark
      export GTK_ICON_THEME_NAME=WhiteSur-dark
      export XDG_CURRENT_DESKTOP=GNOME
      # CRITICAL: Set GDK pixbuf module file to include SVG loader from librsvg
      # Without this, GTK4 cannot render SVG icons on macOS
      export GDK_PIXBUF_MODULEDIR="${gdkPixbufWithSvg}/lib/gdk-pixbuf-2.0/2.10.0/loaders"
      export GDK_PIXBUF_MODULE_FILE="${gdkPixbufWithSvg}/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache"
      # Set DYLD_LIBRARY_PATH so the SVG loader can find librsvg dylib
      export DYLD_LIBRARY_PATH="${pkgs.librsvg}/lib:${pkgs.gdk-pixbuf}/lib:${pkgs.glib}/lib:${pkgs.cairo}/lib:$DYLD_LIBRARY_PATH"
    ''}

    # Enable debug symbols for development
    export RUST_BACKTRACE=1

    # Set up pkg-config paths
    export PKG_CONFIG_PATH="${pkgs.lib.makeSearchPathOutput "dev" "lib/pkgconfig" buildInputs}:$PKG_CONFIG_PATH"

    # SQLx offline mode for development
    export SQLX_OFFLINE=true

    # Platform-specific build configuration
    ${pkgs.lib.optionalString pkgs.stdenv.isLinux ''
      # Force libmpv-sys to use system MPV on Linux
      export MPV_NO_PKG_CONFIG=0
      export DEP_MPV_VERSION_MAJOR=2
      export DEP_MPV_VERSION_MINOR=5
    ''}

    ${pkgs.lib.optionalString pkgs.stdenv.isDarwin ''
      # Fix gettext-sys on macOS
      export GETTEXT_DIR="${pkgs.gettext}"
      export GETTEXT_LIB_DIR="${pkgs.gettext}/lib"
      export GETTEXT_INCLUDE_DIR="${pkgs.gettext}/include"
      export GETTEXT_BIN_DIR="${pkgs.gettext}/bin"
      export GETTEXT_SYSTEM=1
    ''}
  '';

  # Environment variables
  RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;

  # Fix gettext-sys build on macOS
  GETTEXT_DIR =
    if pkgs.stdenv.isDarwin
    then "${pkgs.gettext}"
    else "";
  GETTEXT_LIB_DIR =
    if pkgs.stdenv.isDarwin
    then "${pkgs.gettext}/lib"
    else "";
  GETTEXT_INCLUDE_DIR =
    if pkgs.stdenv.isDarwin
    then "${pkgs.gettext}/include"
    else "";
  GETTEXT_BIN_DIR =
    if pkgs.stdenv.isDarwin
    then "${pkgs.gettext}/bin"
    else "";
  GETTEXT_SYSTEM =
    if pkgs.stdenv.isDarwin
    then "1"
    else "";
}
