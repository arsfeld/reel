# Build the core Zig library (via nix develop)
build:
    nix develop --command zig build

# Run the app (Linux: GTK binary, macOS: Swift app)
run: build
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ "$(uname)" == "Darwin" ]]; then
        eval "$(nix develop --command sh -c 'printf "REEL_MPV_LIBDIR=%q\nREEL_EPOXY_LIBDIR=%q\nREEL_SQLITE_LIBDIR=%q\n" "$REEL_MPV_LIBDIR" "$REEL_EPOXY_LIBDIR" "$REEL_SQLITE_LIBDIR"' 2>/dev/null | grep '^REEL_')"
        cd macos
        # Force re-link when libreel.a changes (Swift PM doesn't detect static lib updates)
        rm -f .build/debug/Reel
        env -i HOME="$HOME" PATH="/usr/bin:/bin:/usr/sbin:/sbin" \
            REEL_MPV_LIBDIR="$REEL_MPV_LIBDIR" \
            REEL_EPOXY_LIBDIR="$REEL_EPOXY_LIBDIR" \
            REEL_SQLITE_LIBDIR="$REEL_SQLITE_LIBDIR" \
            swift run
    else
        nix develop --command ./zig-out/bin/reel
    fi

# Run tests
test:
    nix develop --command zig build test

# Clean build artifacts
clean:
    rm -rf zig-out .zig-cache macos/.build
