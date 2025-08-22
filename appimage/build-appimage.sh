#!/bin/bash
set -e

# Script to build AppImage for Reel
# Requires: appimage-builder or linuxdeploy

VERSION=${1:-$(grep '^version' ../Cargo.toml | cut -d'"' -f2)}
ARCH=$(uname -m)

echo "Building Reel AppImage version $VERSION for $ARCH"

# Create AppDir structure
rm -rf AppDir
mkdir -p AppDir/usr/bin
mkdir -p AppDir/usr/share/applications
mkdir -p AppDir/usr/share/icons/hicolor/scalable/apps
mkdir -p AppDir/usr/share/metainfo

# Copy binary
cp ../target/release/reel AppDir/usr/bin/

# Copy desktop file
cp ../data/dev.arsfeld.Reel.desktop AppDir/usr/share/applications/

# Copy icon
cp ../data/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg AppDir/usr/share/icons/hicolor/scalable/apps/

# Create AppStream metadata
cat > AppDir/usr/share/metainfo/dev.arsfeld.Reel.appdata.xml << EOF
<?xml version="1.0" encoding="UTF-8"?>
<component type="desktop-application">
  <id>dev.arsfeld.Reel</id>
  <metadata_license>CC0-1.0</metadata_license>
  <project_license>GPL-3.0-or-later</project_license>
  <name>Reel</name>
  <summary>Modern media player for GNOME</summary>
  <description>
    <p>Reel is a modern, offline-first media player for the GNOME desktop that provides
    a premium media consumption experience with support for multiple backends including
    Plex, Jellyfin, and local files.</p>
    <p>Features:</p>
    <ul>
      <li>Hardware-accelerated video playback</li>
      <li>Support for Plex, Jellyfin, and local media</li>
      <li>Offline-first architecture with background sync</li>
      <li>Beautiful GTK4/libadwaita interface</li>
      <li>Seamless playback experience</li>
    </ul>
  </description>
  <screenshots>
    <screenshot type="default">
      <caption>Main window</caption>
    </screenshot>
  </screenshots>
  <url type="homepage">https://github.com/arsfeld/reel</url>
  <url type="bugtracker">https://github.com/arsfeld/reel/issues</url>
  <developer_name>Alexandre Rosenfeld</developer_name>
  <releases>
    <release version="$VERSION" date="$(date +%Y-%m-%d)"/>
  </releases>
  <content_rating type="oars-1.1"/>
</component>
EOF

# Method 1: Using linuxdeploy (recommended)
if command -v linuxdeploy-x86_64.AppImage &> /dev/null; then
    echo "Building with linuxdeploy..."
    
    # Download GTK plugin if not present
    if [ ! -f "linuxdeploy-plugin-gtk.sh" ]; then
        wget -q "https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/master/linuxdeploy-plugin-gtk.sh"
        chmod +x linuxdeploy-plugin-gtk.sh
    fi
    
    export VERSION=$VERSION
    linuxdeploy-x86_64.AppImage \
        --appdir AppDir \
        --plugin gtk \
        --output appimage \
        --desktop-file AppDir/usr/share/applications/dev.arsfeld.Reel.desktop \
        --icon-file AppDir/usr/share/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg \
        --executable AppDir/usr/bin/reel
    
    mv Reel*.AppImage "Reel-${VERSION}-${ARCH}.AppImage"
    
# Method 2: Using appimage-builder
elif command -v appimage-builder &> /dev/null; then
    echo "Building with appimage-builder..."
    export VERSION=$VERSION
    appimage-builder --recipe AppImageBuilder.yml
    mv Reel*.AppImage "Reel-${VERSION}-${ARCH}.AppImage"
    
else
    echo "Error: Neither linuxdeploy nor appimage-builder found!"
    echo "Install one of them:"
    echo "  - linuxdeploy: wget https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage"
    echo "  - appimage-builder: pip install appimage-builder"
    exit 1
fi

echo "AppImage built successfully: Reel-${VERSION}-${ARCH}.AppImage"