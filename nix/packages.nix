{
  pkgs,
  rustToolchain,
  buildInputs,
  rustBuildInputs,
  ...
}:
{
  default = pkgs.rustPlatform.buildRustPackage rec {
    pname = "reel";
    version = (builtins.fromTOML (builtins.readFile ../Cargo.toml)).package.version;

    src = ../.;

    cargoLock = {
      lockFile = ../Cargo.lock;
    };

    nativeBuildInputs = rustBuildInputs ++ [ pkgs.mold pkgs.clang ];

    inherit buildInputs;

    # Platform-specific feature flags
    buildNoDefaultFeatures = true;
    buildFeatures =
      if pkgs.stdenv.isDarwin then
        [ "gstreamer" ]  # macOS: GStreamer only (MPV has issues)
      else
        [ "mpv" "gstreamer" ];  # Linux: Both backends

    # Skip tests during build (can be run separately)
    doCheck = false;

    meta = with pkgs.lib; {
      description = "A modern GTK frontend for Plex and other media servers";
      homepage = "https://github.com/arsfeld/reel";
      license = licenses.gpl3Plus;
      maintainers = [];
      platforms = platforms.linux ++ platforms.darwin;
      mainProgram = "reel";
    };
  };

  reel = pkgs.rustPlatform.buildRustPackage rec {
    pname = "reel";
    version = (builtins.fromTOML (builtins.readFile ../Cargo.toml)).package.version;

    src = ../.;

    cargoLock = {
      lockFile = ../Cargo.lock;
    };

    nativeBuildInputs = rustBuildInputs ++ [ pkgs.mold pkgs.clang ];

    inherit buildInputs;

    # Platform-specific feature flags
    buildNoDefaultFeatures = true;
    buildFeatures =
      if pkgs.stdenv.isDarwin then
        [ "gstreamer" ]  # macOS: GStreamer only (MPV has issues)
      else
        [ "mpv" "gstreamer" ];  # Linux: Both backends

    # Skip tests during build (can be run separately)
    doCheck = false;

    meta = with pkgs.lib; {
      description = "A modern GTK frontend for Plex and other media servers";
      homepage = "https://github.com/arsfeld/reel";
      license = licenses.gpl3Plus;
      maintainers = [];
      platforms = platforms.linux ++ platforms.darwin;
      mainProgram = "reel";
    };
  };
}