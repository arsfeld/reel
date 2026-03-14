{
  description = "Reel - A modern, native media player for the Linux desktop";

  inputs = {
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
    flake-utils.lib.eachSystem ["x86_64-linux" "aarch64-linux"] (system: let
      overlays = [
        (import rust-overlay)
      ];
      pkgs = import nixpkgs {
        inherit system overlays;
      };

      rustToolchain = pkgs.rust-bin.stable."1.89.0".default.override {
        extensions = ["rust-src" "rust-analyzer" "rustfmt" "clippy"];
      };

      buildInputs = with pkgs; [
        # GTK and UI
        gtk4
        libadwaita
        adwaita-icon-theme
        hicolor-icon-theme
        libepoxy

        # System libraries
        glib
        cairo
        pango
        gdk-pixbuf
        graphene

        # Media playback
        mpv
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

        meta = with pkgs.lib; {
          description = "A modern, native media player for the Linux desktop";
          license = licenses.gpl3Plus;
          platforms = platforms.linux;
          mainProgram = "reel";
        };
      };
    });
}
