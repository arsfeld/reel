# Build the core Zig library (via nix develop)
build:
    nix develop --command zig build

# Run the macOS Swift app (builds lib first)
# Use env -i to escape the Nix shell environment which conflicts with Xcode's Swift
# Extract Nix library paths first, then run swift outside nix develop so the GUI works
# Remove the linked binary so Swift PM re-links against the fresh libreel.a
run: build
    #!/usr/bin/env bash
    set -euo pipefail
    eval "$(nix develop --command sh -c 'printf "REEL_MPV_LIBDIR=%q\nREEL_EPOXY_LIBDIR=%q\nREEL_SQLITE_LIBDIR=%q\n" "$REEL_MPV_LIBDIR" "$REEL_EPOXY_LIBDIR" "$REEL_SQLITE_LIBDIR"' 2>/dev/null | grep '^REEL_')"
    cd macos
    # Force re-link when libreel.a changes (Swift PM doesn't detect static lib updates)
    rm -f .build/debug/Reel
    env -i HOME="$HOME" PATH="/usr/bin:/bin:/usr/sbin:/sbin" \
        REEL_MPV_LIBDIR="$REEL_MPV_LIBDIR" \
        REEL_EPOXY_LIBDIR="$REEL_EPOXY_LIBDIR" \
        REEL_SQLITE_LIBDIR="$REEL_SQLITE_LIBDIR" \
        swift run

# Run tests
test:
    nix develop --command zig build test

# Clean build artifacts
clean:
    rm -rf zig-out .zig-cache macos/.build
