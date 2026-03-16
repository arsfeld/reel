# Build the core Zig library (via nix develop)
build:
    nix develop --command zig build

# Run the macOS Swift app (builds lib first)
# Use env -i to escape the Nix shell environment which conflicts with Xcode's Swift
run: build
    cd macos && env -i HOME="$HOME" PATH="/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin" swift run

# Run tests
test:
    nix develop --command zig build test

# Clean build artifacts
clean:
    rm -rf zig-out .zig-cache macos/.build
