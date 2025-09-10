# ARM64 Cross-Compilation for GitHub Actions

This document describes the solution for building ARM64 (aarch64) packages on GitHub Actions Ubuntu runners using cross-rs.

## Problem

GitHub Actions Ubuntu runners are x86_64 machines. When trying to build ARM64 packages, several issues arise:

1. **APT Repository Issue**: Ubuntu 24.04 runners fail when adding ARM64 architecture because the default repositories don't contain ARM64 packages
2. **OpenSSL Compilation**: The build fails with `-m64` flag being incorrectly passed to the ARM compiler
3. **Flatpak Runtime**: Flatpak builder needs proper remote configuration for user installations

## Solution: Using cross-rs

### 1. Install cross-rs Tool

[cross-rs](https://github.com/cross-rs/cross) provides zero-setup cross-compilation using Docker containers with all necessary toolchains and libraries pre-configured.

```yaml
- name: Install cargo packaging tools
  run: |
    cargo install cargo-deb --locked || true
    cargo install cargo-generate-rpm --locked || true
    # Install cross for ARM64 cross-compilation
    if [ "${{ matrix.arch }}" = "aarch64" ]; then
      cargo install cross --git https://github.com/cross-rs/cross || true
    fi
```

### 2. Set Up Docker for Cross-Compilation

Docker is needed for the cross-rs containers:

```yaml
- name: Set up Docker for cross-compilation
  if: matrix.arch == 'aarch64'
  uses: docker/setup-buildx-action@v3
```

### 3. Configure Cross-Compilation with Cross.toml

Create a `Cross.toml` file in your project root to configure the cross-compilation environment:

```toml
# Cross.toml
[target.aarch64-unknown-linux-gnu]
image = "ghcr.io/cross-rs/aarch64-unknown-linux-gnu:latest"

pre-build = [
    "dpkg --add-architecture arm64",
    "apt-get update",
    "apt-get install -y --no-install-recommends libgtk-4-dev:arm64 libadwaita-1-dev:arm64 libgstreamer1.0-dev:arm64 libgstreamer-plugins-base1.0-dev:arm64 libmpv-dev:arm64 libsqlite3-dev:arm64 libssl-dev:arm64 libdbus-1-dev:arm64"
]

[target.aarch64-unknown-linux-gnu.env]
passthrough = ["CARGO_TERM_COLOR", "VERSION", "ARCH"]
PKG_CONFIG_ALLOW_CROSS = "1"
PKG_CONFIG_PATH = "/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig"
OPENSSL_DIR = "/usr"
OPENSSL_LIB_DIR = "/usr/lib/aarch64-linux-gnu"
OPENSSL_INCLUDE_DIR = "/usr/include/aarch64-linux-gnu"
```

### 4. Build with cross

Use the `cross` command instead of `cargo` for cross-compilation:

```yaml
- name: Build release binary (cross-compile ARM64)
  if: matrix.arch == 'aarch64'
  run: |
    # Use cross for ARM64 cross-compilation
    cross build --release --target aarch64-unknown-linux-gnu
    # Strip the binary using Docker container
    docker run --rm -v "$PWD":/work -w /work ghcr.io/cross-rs/aarch64-unknown-linux-gnu:latest \
      aarch64-linux-gnu-strip target/aarch64-unknown-linux-gnu/release/reel
```

This approach uses Docker containers with all dependencies pre-installed, eliminating APT repository conflicts.

### 5. Fix Flatpak Remote Configuration

Flatpak needs proper remote configuration for both system and user installations:

```yaml
- name: Install Flatpak and dependencies
  run: |
    sudo apt-get update
    sudo apt-get install -y flatpak flatpak-builder
    
    # Add flathub remote for both system and user
    sudo flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
    flatpak remote-add --user --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
    
    # Update remote metadata
    sudo flatpak update --appstream
    flatpak update --user --appstream
    
    # Install required runtimes
    sudo flatpak install -y flathub org.gnome.Platform//48 org.gnome.Sdk//48 org.freedesktop.Sdk.Extension.rust-stable//23.08
```

### 6. Flatpak Cross-Compilation Environment

When building Flatpak for ARM64, additional environment variables are needed:

```yaml
- name: Build Flatpak
  run: |
    # Set version and architecture environment variables
    export VERSION=${{ needs.create-release.outputs.version }}
    export ARCH=${{ matrix.arch }}
    
    # Set cross-compilation environment for ARM64
    if [ "${{ matrix.arch }}" = "aarch64" ]; then
      export PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig
      export PKG_CONFIG_ALLOW_CROSS=1
      export OPENSSL_DIR=/usr
      export OPENSSL_INCLUDE_DIR=/usr/include
      export OPENSSL_LIB_DIR=/usr/lib/aarch64-linux-gnu
    fi
    
    # Use standalone Flatpak build script
    ./scripts/build-flatpak.sh
```

## Testing Strategy

To avoid creating unnecessary release tags while debugging, implement a test mode:

```yaml
workflow_dispatch:
  inputs:
    tag:
      description: "Release tag"
      required: true
      type: string
    test_mode:
      description: "Test mode - skip creating releases"
      required: false
      default: false
      type: boolean
```

Then conditionally skip release creation:

```yaml
- name: Create Release
  if: ${{ github.event.inputs.test_mode != 'true' }}
  uses: softprops/action-gh-release@v2
  # ...

- name: Upload Artifacts
  if: ${{ github.event.inputs.test_mode != 'true' }}
  # ...
```

### Testing Commands

```bash
# Create a test branch
git checkout -b fix-arm64-build

# Trigger test build (won't create releases)
gh workflow run release.yml --ref fix-arm64-build \
  -f tag=v0.5.0-test1 \
  -f test_mode=true

# Watch the build
gh run watch <run-id>

# Check logs if failed
gh run view <run-id> --log-failed
```

## Key Issues and Solutions

### Previous Issues (Solved by cross-rs)

The following issues are automatically handled by using cross-rs with Docker containers:

1. **"Failed to fetch binary-arm64/Packages 404"** - Docker containers have correct repositories
2. **"error: unrecognized command-line option '-m64'"** - Proper toolchain in container
3. **"cannot find -lgobject-2.0"** - Libraries installed in container
4. **"unmet dependencies"** - No conflicts in isolated container environment

### Remaining Considerations

### Issue 1: "No remote refs found for 'flathub'"
**Cause**: Flatpak remote not properly configured for user installations  
**Solution**: Add flathub remote for both system and user, update appstream metadata

### Issue 2: "expected `*const u8`, found `*const i8`" in Rust code
**Cause**: On ARM64 Linux, `c_char` is `u8` instead of `i8`  
**Solution**: Use `libc::c_char` instead of hardcoding `i8` or `u8` types

### Benefits of cross-rs Approach

1. **Zero-setup**: No need to configure APT repositories or install packages on the host
2. **Reproducible**: Docker containers ensure consistent build environment
3. **Isolated**: No conflicts between host and target architecture packages
4. **Maintained**: cross-rs team maintains the Docker images with updated toolchains
5. **Faster CI**: No need to install packages on every build, they're in the container

## References

- [cross-rs Documentation](https://github.com/cross-rs/cross)
- [cross-rs Wiki](https://github.com/cross-rs/cross/wiki)
- [Docker Hub: cross-rs Images](https://github.com/cross-rs/cross/pkgs/container/aarch64-unknown-linux-gnu)
- [Rust Cross-Compilation](https://rust-lang.github.io/rustup/cross-compilation.html)