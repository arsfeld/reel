{
  description = "Reel - A modern, native media player for the GNOME desktop";

  inputs = {
    # TODO: should be updated when gst-plugins-rs is building again:
    # https://hydra.nixos.org/job/nixpkgs/trunk/gst_all_1.gst-plugins-rs.aarch64-darwin
    nixpkgs.url = "github:NixOS/nixpkgs?rev=51af08a5a2511a027cce68ce2025387983a50f19";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    rust-overlay,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      overlays = [
        (import rust-overlay)
      ];
      pkgs = import nixpkgs {
        inherit system overlays;
      };

      rustToolchain = pkgs.rust-bin.stable."1.89.0".default.override {
        extensions = ["rust-src" "rust-analyzer" "rustfmt" "clippy"];
      };

      # Build inputs needed for compiling the Rust project
      rustBuildInputs = with pkgs;
        [
          rustToolchain
          pkg-config
          desktop-file-utils
        ]
        ++ lib.optionals pkgs.stdenv.isLinux [
          wrapGAppsHook4 # Linux-only for wrapping GTK apps
        ];

      # WhiteSur theme for macOS - download pre-built version to avoid jdupes build issue
      whitesurTheme = pkgs.stdenv.mkDerivation {
        name = "whitesur-gtk-theme-prebuilt";
        src = pkgs.fetchurl {
          url = "https://raw.githubusercontent.com/vinceliuice/WhiteSur-gtk-theme/120d5a1de8f86e42a6e759a4e58df387d536cff2/release/WhiteSur-Dark.tar.xz";
          sha256 = "sha256-wxiFmq17hNbC4VfpJqJ2yWHzVUYvphHHR0wdUHfJQ7U=";
        };

        installPhase = ''
          mkdir -p $out/share/themes
          tar -xf $src -C $out/share/themes
        '';
      };

      # WhiteSur icon theme for macOS - build our own since nixpkgs version is broken on Darwin
      whitesurIconTheme = pkgs.stdenv.mkDerivation {
        name = "whitesur-icon-theme";
        src = pkgs.fetchFromGitHub {
          owner = "vinceliuice";
          repo = "WhiteSur-icon-theme";
          rev = "2025-08-02";
          sha256 = "sha256-pgo3QHsePcIVQtOE953v/HDKpnu7PzruQmpudasrQ3o=";
        };

        nativeBuildInputs = with pkgs; [ gtk3 gnused ];

        # Don't remove icon cache - we need it for GTK to load icons properly
        dontDropIconThemeCache = true;

        installPhase = ''
          # Install WhiteSur (default/light) theme
          THEME_DIR=$out/share/icons/WhiteSur
          mkdir -p $THEME_DIR
          cp -r src/index.theme $THEME_DIR/
          cp -r COPYING AUTHORS $THEME_DIR/
          cp -r src/{actions,animations,apps,categories,devices,emotes,emblems,mimes,places,preferences} $THEME_DIR/
          mkdir -p $THEME_DIR/status
          cp -r src/status/{16,22,24,32,symbolic} $THEME_DIR/status/

          # Install WhiteSur-dark theme (inherits from WhiteSur)
          THEME_DIR=$out/share/icons/WhiteSur-dark
          mkdir -p $THEME_DIR/{apps,categories,emblems,devices,mimes,places,status}
          cp -r src/index.theme $THEME_DIR/
          cp -r COPYING AUTHORS $THEME_DIR/
          cp -r src/actions $THEME_DIR/
          cp -r src/apps $THEME_DIR/
          cp -r src/categories/{22,symbolic} $THEME_DIR/categories/
          cp -r src/emblems/symbolic $THEME_DIR/emblems/
          cp -r src/mimes/symbolic $THEME_DIR/mimes/
          cp -r src/devices/{16,22,24,32,symbolic} $THEME_DIR/devices/
          cp -r src/places/{16,22,24,scalable,symbolic} $THEME_DIR/places/
          cp -r src/status/symbolic $THEME_DIR/status/

          # Update theme Name and add Adwaita to Inherits for proper icon fallback
          ${pkgs.gnused}/bin/sed -e 's/^Name=WhiteSur$/Name=WhiteSur-dark/' \
            -e 's/^Inherits=hicolor,breeze$/Inherits=Adwaita,hicolor,breeze/' \
            $THEME_DIR/index.theme > $THEME_DIR/index.theme.tmp
          mv $THEME_DIR/index.theme.tmp $THEME_DIR/index.theme

          # Generate icon cache for both themes (required for GTK to load icons)
          ${pkgs.gtk3.dev}/bin/gtk-update-icon-cache -q -t -f "$out/share/icons/WhiteSur"
          ${pkgs.gtk3.dev}/bin/gtk-update-icon-cache -q -t -f "$out/share/icons/WhiteSur-dark"
        '';
      };

      # Combined GDK Pixbuf loaders for macOS (includes SVG support)
      # This is needed because gdk-pixbuf doesn't include SVG loader by default
      # and librsvg's loaders.cache doesn't properly merge with gdk-pixbuf's
      gdkPixbufWithSvg = pkgs.stdenv.mkDerivation {
        name = "gdk-pixbuf-with-svg-loader";
        buildInputs = [ pkgs.gdk-pixbuf pkgs.librsvg ];
        nativeBuildInputs = [ pkgs.gdk-pixbuf ];

        buildCommand = ''
          mkdir -p $out/lib/gdk-pixbuf-2.0/2.10.0/loaders

          # Copy all loaders from gdk-pixbuf
          cp -r ${pkgs.gdk-pixbuf}/lib/gdk-pixbuf-2.0/2.10.0/loaders/* $out/lib/gdk-pixbuf-2.0/2.10.0/loaders/ || true

          # Copy SVG loader from librsvg
          cp ${pkgs.librsvg}/lib/gdk-pixbuf-2.0/2.10.0/loaders/libpixbufloader_svg.dylib $out/lib/gdk-pixbuf-2.0/2.10.0/loaders/ || true

          # Generate combined loaders.cache with DYLD_LIBRARY_PATH so SVG loader can find librsvg
          # This is critical on macOS where dylibs have @rpath dependencies
          DYLD_LIBRARY_PATH="${pkgs.librsvg}/lib:${pkgs.gdk-pixbuf}/lib:${pkgs.glib}/lib:${pkgs.cairo}/lib" \
          GDK_PIXBUF_MODULEDIR=$out/lib/gdk-pixbuf-2.0/2.10.0/loaders \
            ${pkgs.gdk-pixbuf.dev}/bin/gdk-pixbuf-query-loaders \
            $out/lib/gdk-pixbuf-2.0/2.10.0/loaders/*.so \
            $out/lib/gdk-pixbuf-2.0/2.10.0/loaders/*.dylib \
            > $out/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache
        '';
      };

      # Platform-specific packages
      linuxOnlyPackages = with pkgs;
        lib.optionals pkgs.stdenv.isLinux [
          gst_all_1.gst-vaapi # VA-API is Linux-only
          mpv # MPV player backend (not working properly on macOS)
        ];

      darwinOnlyPackages = with pkgs;
        lib.optionals pkgs.stdenv.isDarwin [
          whitesurTheme
          whitesurIconTheme
          gdkPixbufWithSvg
        ];

      buildInputs = with pkgs;
        [
          # GTK and UI
          gtk4 # This should be 4.14+ from nixpkgs unstable
          libadwaita
          adwaita-icon-theme # Required for GTK symbolic icons on macOS
          hicolor-icon-theme # Base icon theme for GTK applications
          libepoxy # For OpenGL function loading

          # GStreamer and media
          gst_all_1.gstreamer
          gst_all_1.gst-plugins-base
          gst_all_1.gst-plugins-good
          gst_all_1.gst-plugins-bad
          gst_all_1.gst-plugins-ugly
          gst_all_1.gst-libav
          gst_all_1.gst-plugins-rs # Includes gtk4paintablesink

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
          glib-networking  # CRITICAL: Provides TLS support for libsoup/GStreamer

          # Keyring support
          libsecret
          dbus
          dbus.dev

          # Localization
          gettext

          # Image processing
          librsvg

          jj
        ]
        ++ linuxOnlyPackages ++ darwinOnlyPackages;

      # Import modular configurations
      packages = import ./nix/packages.nix {
        inherit pkgs rustToolchain buildInputs rustBuildInputs;
      };

      devShell = import ./nix/devshell.nix {
        inherit pkgs rustToolchain buildInputs rustBuildInputs whitesurTheme whitesurIconTheme linuxOnlyPackages darwinOnlyPackages gdkPixbufWithSvg;
      };

    in {
      devShells.default = devShell;
      packages = {
        default = packages.default;
        reel = packages.reel;
      };
    });
}
