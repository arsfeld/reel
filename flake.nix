{
  description = "Reel - A modern, native media player for the Linux desktop";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
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
    flake-utils.lib.eachSystem ["x86_64-linux" "aarch64-linux"] (system: let
      overlays = [
        (import rust-overlay)
      ];
      pkgs = import nixpkgs {
        inherit system overlays;
      };

      rustToolchain = pkgs.rust-bin.stable."1.94.0".default.override {
        extensions = ["rust-src" "rust-analyzer" "rustfmt" "clippy"];
      };

      gstPlugins = with pkgs.gst_all_1; [
        gstreamer
        gst-plugins-base
        gst-plugins-good
        gst-plugins-bad
        gst-plugins-ugly
        gst-libav
        gst-plugins-rs
      ];

      # gstreamer splits bin/lib outputs; getLib resolves the store path that
      # actually contains lib/gstreamer-1.0 (coreelements, typefind, etc.).
      gstPluginPath = pkgs.lib.concatStringsSep ":" (
        map (pkg: "${pkgs.lib.getLib pkg}/lib/gstreamer-1.0") gstPlugins
      );

      buildInputs = with pkgs; [
        # GTK and UI
        gtk4
        libadwaita
        adwaita-icon-theme
        hicolor-icon-theme
        libepoxy
        mpv

        # OpenGL / EGL (may be needed by GStreamer GL sinks)
        libGL
        mesa  # provides actual GL/EGL drivers + vendor JSON for libglvnd dispatch

        # System libraries
        glib
        cairo
        pango
        gdk-pixbuf
        graphene

        # Media playback (GStreamer)
        gst_all_1.gstreamer
        gst_all_1.gst-plugins-base
        gst_all_1.gst-plugins-good
        gst_all_1.gst-plugins-bad
        gst_all_1.gst-plugins-ugly
        gst_all_1.gst-libav
        gst_all_1.gst-plugins-rs
      ];

      nativeBuildInputs = [
        rustToolchain
        pkgs.pkg-config
        pkgs.wrapGAppsHook4
      ];

    in {
      devShells.default = pkgs.mkShell {
        inherit buildInputs nativeBuildInputs;

        packages = with pkgs; [
          cargo-watch
          mold
          clang
          # Dev-time only: generate migrations + regenerate src/schema.rs.
          # The app itself self-migrates via embed_migrations! (no runtime CLI).
          (diesel-cli.override {
            sqliteSupport = true;
            postgresqlSupport = false;
            mysqlSupport = false;
          })
        ];

        shellHook = ''
          # Build library search paths for all buildInputs
          LIB_PATHS="${pkgs.lib.makeLibraryPath buildInputs}"
          LINK_ARGS=()
          for lib in ''${LIB_PATHS//:/ }; do
            LINK_ARGS+=("-C link-arg=-L$lib")
          done

          # Set target-specific RUSTFLAGS which merge with config file rustflags
          export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="''${LINK_ARGS[*]}"
          export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS="''${LINK_ARGS[*]}"

          # Set up GTK schema paths
          export XDG_DATA_DIRS="${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk4}/share/gsettings-schemas/${pkgs.gtk4.name}:$XDG_DATA_DIRS"

          # Wire Mesa EGL vendor so libglvnd can dispatch to GPU drivers
          export __EGL_VENDOR_LIBRARY_DIRS="${pkgs.mesa}/share/glvnd/egl_vendor.d"

          # GStreamer plugin search path (includes gtk4paintablesink from gst-plugins-rs)
          export GST_PLUGIN_SYSTEM_PATH_1_0="${gstPluginPath}"

          export RUST_BACKTRACE=1
          export PKG_CONFIG_PATH="${pkgs.lib.makeSearchPathOutput "dev" "lib/pkgconfig" buildInputs}:$PKG_CONFIG_PATH"
        '';

        RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;
      };

      packages.default = pkgs.rustPlatform.buildRustPackage {
        pname = "reel";
        version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;

        src = ./.;

        cargoLock = {
          lockFile = ./Cargo.lock;
        };

        nativeBuildInputs = nativeBuildInputs ++ [pkgs.mold pkgs.clang];

        inherit buildInputs;

        doCheck = false;

        preFixup = ''
          gappsWrapperArgs+=(
            --set GST_PLUGIN_SYSTEM_PATH_1_0 "${gstPluginPath}"
          )
        '';

        meta = with pkgs.lib; {
          description = "A modern, native media player for the Linux desktop";
          license = licenses.gpl3Plus;
          platforms = platforms.linux;
          mainProgram = "reel";
        };
      };
    });
}
