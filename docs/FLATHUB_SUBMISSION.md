# Flathub Submission Guide

This document provides instructions for submitting Reel to Flathub, the official Flatpak repository.

## Prerequisites

Before submitting to Flathub, ensure you have:

1. A GitHub account with access to create repositories
2. Flatpak and flatpak-builder installed locally for testing
3. All Flathub requirements met (see checklist below)

## Flathub Requirements Checklist

### Manifest Requirements
- ✅ Manifest file (`dev.arsfeld.Reel.json`) is properly formatted
- ✅ Uses GNOME runtime version 48 (hosted on Flathub)
- ✅ Includes cargo sources via `cargo-sources.json`
- ✅ Blueprint compiler module removed (not needed)
- ✅ Proper cleanup rules for development files

### Metadata Requirements
- ✅ AppStream metainfo file (`data/dev.arsfeld.Reel.metainfo.xml`) validates successfully
- ✅ Desktop file (`data/dev.arsfeld.Reel.desktop`) follows standards
- ✅ SVG icon properly installed
- ✅ LICENSE file installed to `/app/share/licenses/dev.arsfeld.Reel/`

### Permissions (finish-args)
- ✅ `--share=network` - Required for Plex/Jellyfin API access
- ✅ `--share=ipc` - Required for GTK/Wayland
- ✅ `--socket=fallback-x11` - X11 fallback support
- ✅ `--socket=wayland` - Wayland display protocol
- ✅ `--socket=pulseaudio` - Audio playback
- ✅ `--device=dri` - Hardware-accelerated video rendering

### Architecture Support
- ✅ `flathub.json` specifies x86_64 and aarch64 support

## Pre-Submission Testing

### 1. Generate Cargo Sources

Before building, ensure cargo-sources.json is up to date:

```bash
# Using the included generator script
python3 flatpak-cargo-generator.py ./Cargo.lock -o cargo-sources.json
```

### 2. Validate Metadata

```bash
# Validate AppStream metadata
appstreamcli validate data/dev.arsfeld.Reel.metainfo.xml

# Expected output: "Validation was successful"
```

### 3. Build Locally

Test the flatpak build locally before submitting:

```bash
# Build using the included script
./scripts/build-flatpak.sh

# Or manually:
flatpak-builder --user --install --force-clean build-dir dev.arsfeld.Reel.json
```

### 4. Test the Application

```bash
# Run the flatpak
flatpak run dev.arsfeld.Reel

# Test in development mode
flatpak run --devel dev.arsfeld.Reel
```

### 5. Verify File Installation

Check that all required files are properly installed:

```bash
# Check installed files
flatpak run --command=sh dev.arsfeld.Reel
ls /app/bin/reel
ls /app/share/applications/dev.arsfeld.Reel.desktop
ls /app/share/metainfo/dev.arsfeld.Reel.metainfo.xml
ls /app/share/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg
ls /app/share/licenses/dev.arsfeld.Reel/LICENSE
```

## Flathub Submission Process

### 1. Fork the Flathub Repository

Create a fork of the Flathub repository template:

```bash
# Visit https://github.com/flathub/flathub
# Click "Fork" to create your submission repository
```

### 2. Create Application Repository

1. Go to https://github.com/flathub/flathub
2. Follow the instructions to request a new app repository
3. Flathub maintainers will create `flathub/dev.arsfeld.Reel` repository

### 3. Prepare Submission Branch

In the newly created repository:

```bash
# Clone your Flathub app repository
git clone git@github.com:flathub/dev.arsfeld.Reel.git
cd dev.arsfeld.Reel

# Copy required files
cp /path/to/reel/dev.arsfeld.Reel.json .
cp /path/to/reel/flathub.json .
cp /path/to/reel/cargo-sources.json .

# Commit and push
git add .
git commit -m "Initial Flathub submission for Reel v0.7.5"
git push origin master
```

### 4. Create Pull Request

1. The push will automatically trigger Flathub CI checks
2. Monitor the CI build at https://flathub.org/builds
3. Fix any issues reported by CI
4. Wait for Flathub maintainers to review

### 5. Address Review Feedback

Flathub reviewers may request changes:

- Security/permission improvements
- Metadata enhancements
- Build process clarifications
- License verification

Make requested changes and push updates to the same branch.

## Post-Submission Updates

### Updating to New Versions

When releasing a new version of Reel:

1. **Update Cargo Sources**:
   ```bash
   python3 flatpak-cargo-generator.py ./Cargo.lock -o cargo-sources.json
   ```

2. **Update Manifest**:
   - Change `tag` in `dev.arsfeld.Reel.json` to new version
   - Update any dependency versions if needed

3. **Update Metadata**:
   - Add new `<release>` entry in `data/dev.arsfeld.Reel.metainfo.xml`
   - Include release date and changelog

4. **Submit Update PR**:
   ```bash
   git checkout -b update-v0.8.0
   git add dev.arsfeld.Reel.json cargo-sources.json
   git commit -m "Update to v0.8.0"
   git push origin update-v0.8.0
   # Create pull request on GitHub
   ```

## Flathub Build Process

The Flathub build system will:

1. Clone the specified git tag from GitHub
2. Download all cargo dependencies from crates.io
3. Build the application in a sandboxed environment
4. Run automated tests and security checks
5. Publish to Flathub if all checks pass

## Useful Resources

- [Flathub Documentation](https://docs.flathub.org/)
- [Flathub Requirements](https://docs.flathub.org/docs/for-app-authors/requirements)
- [AppStream Metadata](https://www.freedesktop.org/software/appstream/docs/)
- [Flatpak Builder Tools](https://github.com/flatpak/flatpak-builder-tools)
- [Flathub Quality Guidelines](https://docs.flathub.org/docs/for-app-authors/metainfo-guidelines/)

## Troubleshooting

### Build Fails with Cargo Error

Ensure `cargo-sources.json` is generated from the exact `Cargo.lock` matching the git tag:

```bash
git checkout v0.7.5
python3 flatpak-cargo-generator.py ./Cargo.lock -o cargo-sources.json
```

### AppStream Validation Fails

Run validation locally and fix issues:

```bash
appstreamcli validate data/dev.arsfeld.Reel.metainfo.xml
```

### Permission Denied Errors

Review `finish-args` in the manifest and ensure all required permissions are included while keeping them minimal.

### Icon Not Displaying

Verify icon installation path matches the icon name in the desktop file and metainfo.xml:
- Icon file: `/app/share/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg`
- Icon name in desktop file: `dev.arsfeld.Reel`

## Support

For Flathub-specific questions:
- [Flathub Discourse](https://discourse.flathub.org/)
- [Flathub Matrix Channel](https://matrix.to/#/#flathub:matrix.org)

For Reel-specific questions:
- [GitHub Issues](https://github.com/arsfeld/reel/issues)
