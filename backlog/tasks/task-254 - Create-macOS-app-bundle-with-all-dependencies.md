---
id: task-254
title: Create macOS app bundle with all dependencies
status: Done
assignee:
  - '@claude'
created_date: '2025-09-26 14:27'
updated_date: '2025-09-26 15:42'
labels:
  - macos
  - packaging
dependencies: []
priority: high
---

## Description

Build a proper macOS .app bundle that includes all necessary dependencies (whitesur-gtk-theme, GTK, libmpv2, etc.) and add a command to flake.nix to automate the build process

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Create macOS app bundle structure with proper Info.plist
- [x] #2 Bundle all required dylibs and frameworks (GTK4, libadwaita, MPV, GStreamer)
- [x] #3 Include whitesur-gtk-theme and other macOS-specific resources
- [x] #4 Handle library path resolution and code signing requirements
- [x] #5 Add build-macos-bundle command to flake.nix
- [x] #6 Test app bundle runs independently without Nix environment
<!-- AC:END -->


## Implementation Plan

1. Research macOS app bundle structure and cargo-bundle requirements
2. Create Cargo.toml configuration for cargo-bundle
3. Implement build script that handles all dependencies and library paths
4. Add GTK theme and resources to bundle
5. Create Info.plist with proper metadata
6. Add build-macos-bundle command to flake.nix
7. Test bundle runs without Nix environment

## Implementation Notes

Implemented comprehensive macOS app bundle support:

1. Added cargo-bundle configuration to Cargo.toml with:
   - Bundle metadata (name, identifier, category)
   - Icon configuration pointing to data/macos/icon.icns
   - macOS frameworks and URL schemes
   - Resource bundling for themes and styles

2. Created scripts/build-macos-bundle.sh that:
   - Generates ICNS icon from SVG using rsvg-convert
   - Builds release binary with cargo
   - Creates app bundle using cargo-bundle
   - Post-processes bundle with launcher script for environment setup
   - Bundles dynamic libraries using dylibbundler
   - Handles code signing if identity available
   - Optionally creates DMG for distribution

3. Updated flake.nix with:
   - macOSBundleTools list including cargo-bundle, librsvg, imagemagick, dylibbundler
   - build-macos-bundle command that sets up environment and runs build script
   - Added tools to devShells packages for macOS
   - WhiteSur GTK theme configuration for macOS

All dependencies are managed through Nix, ensuring reproducible builds without manual tool installation.
