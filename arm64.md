# ARM64 Cross-Compilation for GitHub Actions

This document describes the solution for building ARM64 (aarch64) packages on GitHub Actions Ubuntu runners.

## Problem

GitHub Actions Ubuntu runners are x86_64 machines. When trying to build ARM64 packages, several issues arise:

1. **APT Repository Issue**: Ubuntu 24.04 runners fail when adding ARM64 architecture because the default repositories don't contain ARM64 packages
2. **OpenSSL Compilation**: The build fails with `-m64` flag being incorrectly passed to the ARM compiler
3. **Flatpak Runtime**: Flatpak builder needs proper remote configuration for user installations

## Solution

### 1. Configure APT Sources for Multi-Architecture

Ubuntu 24.04 uses the new deb822 format for package sources. ARM64 packages are hosted on `ports.ubuntu.com`, not the standard mirrors.

```yaml
- name: Install cross-compilation dependencies for ARM64
  if: matrix.arch == 'aarch64'
  run: |
    # Enable ARM64 architecture
    sudo dpkg --add-architecture arm64
    
    # Remove the default Ubuntu sources that don't support ARM64
    sudo rm -f /etc/apt/sources.list.d/ubuntu.sources
    
    # Create proper deb822 format sources for both amd64 and arm64
    cat <<EOF | sudo tee /etc/apt/sources.list.d/amd64.sources
    Types: deb
    URIs: http://azure.archive.ubuntu.com/ubuntu/
    Suites: noble noble-updates noble-backports
    Components: main restricted universe multiverse
    Architectures: amd64
    
    Types: deb
    URIs: http://security.ubuntu.com/ubuntu/
    Suites: noble-security
    Components: main restricted universe multiverse
    Architectures: amd64
    EOF
    
    cat <<EOF | sudo tee /etc/apt/sources.list.d/arm64.sources
    Types: deb
    URIs: http://ports.ubuntu.com/ubuntu-ports/
    Suites: noble noble-updates noble-backports noble-security
    Components: main restricted universe multiverse
    Architectures: arm64
    EOF
    
    sudo apt-get update
    
    # Install cross-compilation tools and ARM64 libraries
    sudo apt-get install -y \
      gcc-aarch64-linux-gnu \
      g++-aarch64-linux-gnu \
      pkg-config \
      libssl-dev:arm64 \
      libgtk-4-dev:arm64 \
      libadwaita-1-dev:arm64 \
      libgstreamer1.0-dev:arm64 \
      libgstreamer-plugins-base1.0-dev:arm64 \
      libmpv-dev:arm64 \
      libsqlite3-dev:arm64 \
      libdbus-1-dev:arm64 \
      libglib2.0-dev:arm64 \
      libcairo2-dev:arm64 \
      libpango1.0-dev:arm64
```

### 2. Set Up QEMU for ARM64 Emulation

QEMU is needed for running ARM64 binaries during the build process:

```yaml
- name: Set up QEMU for cross-compilation
  if: matrix.arch == 'aarch64'
  uses: docker/setup-qemu-action@v3
  with:
    platforms: arm64
```

### 3. Configure Cross-Compilation Environment

**Simplified Approach**: Create a temporary Cargo configuration file instead of setting many environment variables:

```yaml
- name: Setup Cargo config for ARM64 cross-compilation
  if: matrix.arch == 'aarch64'
  run: |
    mkdir -p .cargo
    cat > .cargo/config.toml << 'EOF'
    [target.aarch64-unknown-linux-gnu]
    linker = "aarch64-linux-gnu-gcc"
    
    [env]
    PKG_CONFIG_ALLOW_CROSS = "1"
    PKG_CONFIG_PATH = "/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig"
    OPENSSL_DIR = "/usr"
    OPENSSL_LIB_DIR = "/usr/lib/aarch64-linux-gnu"
    OPENSSL_INCLUDE_DIR = "/usr/include/aarch64-linux-gnu"
    EOF

- name: Build release binary (cross-compile ARM64)
  if: matrix.arch == 'aarch64'
  run: |
    cargo build --release --target aarch64-unknown-linux-gnu
    aarch64-linux-gnu-strip target/aarch64-unknown-linux-gnu/release/reel
```

This approach is cleaner and keeps all cross-compilation settings in one place. The `.cargo/config.toml` file is created only in CI and won't interfere with local development.

### 4. Fix Flatpak Remote Configuration

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

### 5. Flatpak Cross-Compilation Environment

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

### Issue 1: "Failed to fetch binary-arm64/Packages 404"
**Cause**: Default Ubuntu mirrors don't host ARM64 packages  
**Solution**: Use `ports.ubuntu.com` for ARM64 packages with deb822 format

### Issue 2: "error: unrecognized command-line option '-m64'"
**Cause**: x86_64 flags being passed to ARM compiler  
**Solution**: Set proper OpenSSL environment variables and HOST_CC/TARGET_CC

### Issue 3: "No remote refs found for 'flathub'"
**Cause**: Flatpak remote not properly configured for user installations  
**Solution**: Add flathub remote for both system and user, update appstream metadata

### Issue 4: "cannot find -lgobject-2.0" and other libraries
**Cause**: Linker finding x86_64 libraries instead of ARM64 ones  
**Solution**: Install ARM64 versions of all required libraries (GTK4, GLib, etc.)

### Issue 5: "expected `*const u8`, found `*const i8`" in Rust code
**Cause**: On ARM64 Linux, `c_char` is `u8` instead of `i8`  
**Solution**: Use `libc::c_char` instead of hardcoding `i8` or `u8` types

## References

- [GitHub Issue: apt update fails on Ubuntu 24.04 with ARM64](https://github.com/actions/runner-images/issues/10901)
- [Ubuntu Ports Repository](http://ports.ubuntu.com/)
- [Rust Cross-Compilation](https://rust-lang.github.io/rustup/cross-compilation.html)
- [OpenSSL Cross-Compilation](https://github.com/sfackler/rust-openssl/issues/1592)