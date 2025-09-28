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
          url = "https://github.com/vinceliuice/WhiteSur-gtk-theme/raw/master/release/WhiteSur-Dark.tar.xz";
          sha256 = "sha256-wxiFmq17hNbC4VfpJqJ2yWHzVUYvphHHR0wdUHfJQ7U=";
        };

        installPhase = ''
          mkdir -p $out/share/themes
          tar -xf $src -C $out/share/themes
        '';
      };

      # Platform-specific packages
      linuxOnlyPackages = with pkgs;
        lib.optionals pkgs.stdenv.isLinux [
          gst_all_1.gst-vaapi # VA-API is Linux-only
        ];

      darwinOnlyPackages = with pkgs;
        lib.optionals pkgs.stdenv.isDarwin [
          whitesurTheme
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
        inherit pkgs rustToolchain buildInputs rustBuildInputs whitesurTheme linuxOnlyPackages darwinOnlyPackages;
      };

    in {
      devShells.default = devShell;
      packages = {
        default = packages.default;
        reel = packages.reel;
      };
    });
}