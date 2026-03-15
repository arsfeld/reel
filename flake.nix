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
            gtk4
            libadwaita
            sqlite
            libepoxy
            glib
            gobject-introspection
          ];

          shellHook = ''
            echo "Reel dev environment ready"
            echo "  Zig: $(zig version)"
            echo "  mpv: ${pkgs.mpv-unwrapped.version}"
            echo "  GTK4: ${pkgs.gtk4.version}"
            echo "  libadwaita: ${pkgs.libadwaita.version}"
          '';
        };
      }
    );
}
