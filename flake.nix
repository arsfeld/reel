{
  description = "Reel - Native media center written in Zig";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        devShells.default = pkgs.mkShell {
          hardeningDisable = [ "all" ];

          nativeBuildInputs = with pkgs; [
            zig
            pkg-config
          ];

          buildInputs = with pkgs; [
            mpv-unwrapped
            sqlite
            libepoxy
            glib
          ] ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
            gtk4
            libadwaita
            gobject-introspection
          ];

          shellHook = ''
            echo "Reel dev environment ready"
            echo "  Zig: $(zig version)"
            echo "  mpv: ${pkgs.mpv-unwrapped.version}"
          '' + pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isDarwin ''
            export REEL_MPV_LIBDIR="${pkgs.mpv-unwrapped}/lib"
            export REEL_EPOXY_LIBDIR="${pkgs.libepoxy}/lib"
            export REEL_SQLITE_LIBDIR="${pkgs.sqlite.out}/lib"
          '' + pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux ''
            echo "  GTK4: ${pkgs.gtk4.version}"
            echo "  libadwaita: ${pkgs.libadwaita.version}"
          '';
        };
      }
    );
}
