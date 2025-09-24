# Reel Build Scripts

This directory contains Docker-based build and test scripts for building Reel packages across multiple architectures and distributions.

## Overview

The build system uses Docker with cargo-chef for efficient dependency caching, allowing you to build:
- **Binary executables** for direct distribution
- **Debian packages** (.deb) for Ubuntu/Debian systems
- **RPM packages** (.rpm) for Fedora/RHEL systems
- **AppImages** for universal Linux distribution
- **Flatpak packages** for sandboxed distribution

All builds support both **x86_64** and **arm64** architectures through Docker buildx.

## Prerequisites

- Docker with buildx support (Docker Desktop or Docker CE with buildx plugin)
- Git
- Basic build tools (make, etc.)

## Quick Start

```bash
# Build all packages for x86_64
./scripts/build-with-docker.sh x86_64 all

# Test the built packages
./scripts/test-packages.sh
```

## Docker-Based Build System

### Main Build Script: `build-with-docker.sh`

Builds Reel packages using Docker for consistent, reproducible builds.

**Usage:**
```bash
./scripts/build-with-docker.sh <arch> <build-type>
```

**Arguments:**
- `arch`: Target architecture (`x86_64`, `amd64`, `aarch64`, `arm64`)
- `build-type`: What to build (`all`, `binary`, `deb`, `rpm`, `appimage`)

**Examples:**
```bash
# Build only the binary for x86_64
./scripts/build-with-docker.sh x86_64 binary

# Build Debian package for ARM64
./scripts/build-with-docker.sh arm64 deb

# Build everything for x86_64
./scripts/build-with-docker.sh x86_64 all
```

**Output files** (in project root):
- `reel-linux-<arch>` - Standalone binary
- `reel-<version>-<arch>.deb` - Debian package
- `reel-<version>-<arch>.rpm` - RPM package
- `reel-<version>-<arch>.AppImage` - AppImage

### Build Dockerfile: `Dockerfile.build`

Multi-stage Dockerfile using Ubuntu 24.04 as base (for GTK 4.14+ support).

**Key features:**
- Uses **cargo-chef** for dependency caching
- Includes all build dependencies (clang, mold, pkg-config, etc.)
- Optimized layer caching for faster rebuilds
- Supports cross-architecture builds via Docker buildx

**Build stages:**
1. `base` - Ubuntu 24.04 with all system dependencies
2. `planner` - Generates cargo-chef recipe for dependencies
3. `cacher` - Builds and caches Rust dependencies
4. `builder` - Builds the final application

## Testing System

### Test Script: `test-packages.sh`

Validates built packages work correctly in Ubuntu and Fedora containers.

**What it tests:**
1. Binary execution with runtime dependencies
2. Package installation (.deb on Ubuntu, .rpm on Fedora)
3. AppImage format validation and execution

**Usage:**
```bash
./scripts/test-packages.sh
```

### Test Dockerfiles

**`Dockerfile.test-ubuntu`** - Ubuntu 24.04 with:
- GTK4 and libadwaita runtime
- GStreamer plugins
- MPV libraries (libmpv2)
- Test utilities (file, etc.)

**`Dockerfile.test-fedora`** - Fedora 40 with:
- GTK4 and libadwaita runtime
- GStreamer plugins
- MPV libraries
- Test utilities

## Package-Specific Scripts

### AppImage: `build-appimage.sh`

Creates an AppImage package that runs on most Linux distributions.

**Usage:**
```bash
# From project root
./scripts/build-appimage.sh

# Or set custom version/architecture
VERSION=1.0.0 ARCH=x86_64 ./scripts/build-appimage.sh
```

**Requirements:**
- Compiled release binary at `target/release/reel`
- Desktop file and icon in `data/` directory
- Internet connection (downloads LinuxDeploy tools)

**Output:** `reel-${VERSION}-${ARCH}.AppImage`

### Flatpak: `build-flatpak.sh`

Creates a Flatpak package with both bundle and repository formats.

**Usage:**
```bash
# From project root
./scripts/build-flatpak.sh

# Or set custom version
VERSION=1.0.0 ./scripts/build-flatpak.sh
```

**Requirements:**
- `flatpak-builder` installed
- GNOME Platform/SDK 48 runtimes
- Internet connection (downloads dependencies)
- Valid Flatpak manifest (`dev.arsfeld.Reel.json`)

**Output:**
- `reel-${VERSION}-x86_64.flatpak` - Installable bundle
- `repo/` - OSTree repository directory

### Flatpak Remote Install: `install-flatpak-remote.sh`

Interactive script for end users to install Reel from GitHub releases.

**Usage:**
```bash
# Install latest version
./scripts/install-flatpak-remote.sh

# Install specific version
./scripts/install-flatpak-remote.sh v0.4.0
```

## Architecture Support

### x86_64 (Intel/AMD)

Primary architecture with full support for all package types.

```bash
./scripts/build-with-docker.sh x86_64 all
```

### ARM64/aarch64

ARM architecture support for Raspberry Pi, Apple Silicon (via virtualization), etc.

```bash
./scripts/build-with-docker.sh arm64 all
```

**Note:** ARM64 builds use QEMU emulation on x86_64 hosts, which is slower but produces native ARM binaries.

## Caching and Performance

The build system uses several caching strategies:

1. **Docker layer caching** - System dependencies are cached in Docker layers
2. **cargo-chef** - Rust dependencies are pre-built and cached
3. **Docker buildx cache** - Build cache can be exported/imported

### Speeding up builds

After the first build, subsequent builds are much faster:
- Changing only source code: ~2-3 minutes
- Adding new dependencies: ~5-10 minutes
- First build: ~15-20 minutes

### Cache management

```bash
# Clear Docker build cache
docker buildx prune

# Remove test images
docker rmi reel-test-ubuntu:24.04 reel-test-fedora:40

# Remove build images
docker rmi reel-build:x86_64 reel-build:aarch64
```

## GitHub Actions Integration

These scripts are used in the `.github/workflows/release.yml` workflow:

```yaml
# Example workflow snippet
- name: Build packages
  run: ./scripts/build-with-docker.sh x86_64 all

- name: Test packages
  run: ./scripts/test-packages.sh

- name: Upload artifacts
  uses: actions/upload-artifact@v3
  with:
    path: |
      reel-linux-*
      reel-*.deb
      reel-*.rpm
      reel-*.AppImage
```

## Troubleshooting

### Common Build Issues

**Issue:** Build fails with "cargo-chef cook failed"
**Solution:** Clear cache with `docker buildx prune` and rebuild

**Issue:** Cross-architecture build is very slow
**Solution:** This is normal with QEMU emulation. Consider using native ARM hardware or cloud builders.

**Issue:** Missing dependencies error
**Solution:** Ensure Docker is installed and running, check that all required files (LICENSE, README.md) exist

### Package Testing Failures

**Issue:** Binary fails with missing shared libraries
**Solution:** Test containers need runtime dependencies - check Dockerfile.test-* files

**Issue:** AppImage won't run in container
**Solution:** AppImage needs FUSE support - use `--appimage-extract-and-run` flag

**Issue:** Package fails to install
**Solution:** Check that all runtime dependencies are included in test Dockerfiles

### Debugging

Add verbose output to any script:
```bash
# Enable debug mode
bash -x ./scripts/build-with-docker.sh x86_64 all

# Or modify script
sed -i '2a set -x' scripts/build-with-docker.sh
```

Check Docker build logs:
```bash
# View detailed build output
docker buildx build --progress=plain -f scripts/Dockerfile.build .
```

## Development Workflow

1. **Make code changes** in your editor
2. **Build packages** using Docker scripts (dependencies cached)
3. **Test packages** in clean containers
4. **Commit changes** when tests pass
5. **CI/CD builds** automatically on push

## Contributing

When modifying the build system:

1. **Test locally first** - Ensure builds work on your machine
2. **Update documentation** - Keep this README current
3. **Preserve caching** - Don't break cargo-chef stages
4. **Test all architectures** - Verify x86_64 and arm64 if possible
5. **Validate packages** - Run test script after changes

### Adding a new package format

1. Update `Dockerfile.build` if new dependencies needed
2. Add build logic to `build-with-docker.sh`
3. Create test cases in `test-packages.sh`
4. Update this README with instructions

## License

These build scripts are part of the Reel project and follow the same license.