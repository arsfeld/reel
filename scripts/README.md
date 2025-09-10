# Build Scripts

This directory contains build scripts for creating different package formats of Reel.

## Available Scripts

### `build-appimage.sh`
Creates an AppImage package that runs on most Linux distributions.

**Usage:**
```bash
# From project root
./scripts/build-appimage.sh

# Or set custom version
VERSION=1.0.0 ./scripts/build-appimage.sh
```

**Requirements:**
- Compiled release binary at `target/release/reel`
- Desktop file and icon in `data/` directory
- Internet connection (downloads LinuxDeploy tools)

**Output:** `reel-${VERSION}-x86_64.AppImage`

### `build-flatpak.sh`
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
- `flatpak-install-instructions.md` - User installation guide

### `install-flatpak-remote.sh`
Interactive script for end users to install Reel from GitHub releases.

**Usage:**
```bash
# Install latest version
./scripts/install-flatpak-remote.sh

# Install specific version
./scripts/install-flatpak-remote.sh v0.4.0
```

**Features:**
- Auto-detects latest release
- Choose between bundle or remote installation
- Creates desktop shortcut
- Handles dependencies automatically

## GitHub Actions Integration

These scripts are used in the `.github/workflows/release.yml` workflow:

- **AppImage**: Called from `build-linux` job
- **Flatpak**: Called from `build-flatpak` job
- **Install script**: Included as release asset for users

## Development Workflow

1. **Local testing**: Run scripts manually during development
2. **Version control**: Scripts automatically detect version from `Cargo.toml`
3. **CI/CD**: GitHub Actions sets `VERSION` environment variable for releases
4. **Distribution**: All outputs are attached to GitHub releases

## Troubleshooting

### Common Issues

**AppImage fails to run:**
- Check if FUSE is available: `fusermount --version`
- Try extracting manually: `./reel-*.AppImage --appimage-extract`

**Flatpak build fails:**
- Ensure Flathub is added: `flatpak remote-list`
- Install required runtimes manually
- Check disk space (builds can be 1GB+)

**Permission errors:**
- Make scripts executable: `chmod +x scripts/*.sh`
- Use `--user` flag for Flatpak commands

### Debugging

Add `set -x` to any script for verbose output:
```bash
# Enable debug mode
sed -i '2a set -x' scripts/build-flatpak.sh
```

For more information, see the main project documentation.