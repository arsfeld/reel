#!/usr/bin/env bash
set -e

echo "Building AppImage..."

VERSION=${VERSION:-$(grep '^version' Cargo.toml | cut -d'"' -f2)}
export VERSION
export DEPLOY_GTK_VERSION=4
export LINUXDEPLOY_OUTPUT_VERSION=$VERSION

echo "Building AppImage for version: $VERSION"

# Clean up previous builds
rm -rf AppDir
rm -rf squashfs-root
rm -f *.AppImage
rm -f linuxdeploy-*.AppImage
rm -f linuxdeploy-plugin-*.sh

# Download AppImage tools
echo "=== Setting up AppImage tools ==="
wget -q https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
chmod +x linuxdeploy-x86_64.AppImage

# Extract LinuxDeploy AppImage if FUSE is not available
if ! ./linuxdeploy-x86_64.AppImage --help &>/dev/null; then
  echo "FUSE not available, extracting LinuxDeploy AppImage..."
  ./linuxdeploy-x86_64.AppImage --appimage-extract &>/dev/null
  LINUXDEPLOY="./squashfs-root/AppRun"
else
  LINUXDEPLOY="./linuxdeploy-x86_64.AppImage"
fi

wget -q https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/master/linuxdeploy-plugin-gtk.sh
chmod +x linuxdeploy-plugin-gtk.sh
wget -q https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gstreamer/master/linuxdeploy-plugin-gstreamer.sh
chmod +x linuxdeploy-plugin-gstreamer.sh

# Create AppDir structure
echo "=== Creating AppDir structure ==="
mkdir -p AppDir/usr/bin
mkdir -p AppDir/usr/share/applications
mkdir -p AppDir/usr/share/icons/hicolor/scalable/apps

# Copy our files (assumes release binary already built)
cp target/release/reel AppDir/usr/bin/
chmod +x AppDir/usr/bin/reel
cp data/dev.arsfeld.Reel.desktop AppDir/usr/share/applications/
cp data/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg AppDir/usr/share/icons/hicolor/scalable/apps/

# Set library paths for dependency detection (Ubuntu standard paths)
export LD_LIBRARY_PATH="/usr/lib/x86_64-linux-gnu:$LD_LIBRARY_PATH"

echo "=== Running LinuxDeploy with GTK4 and GStreamer plugins ==="
$LINUXDEPLOY \
  --appdir AppDir \
  --executable AppDir/usr/bin/reel \
  --desktop-file AppDir/usr/share/applications/dev.arsfeld.Reel.desktop \
  --icon-file AppDir/usr/share/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg \
  --plugin gtk \
  --plugin gstreamer \
  --output appimage

# Rename the output to our convention
echo "=== Renaming AppImage ==="
if ls Reel-*.AppImage 1> /dev/null 2>&1; then
  mv Reel-*.AppImage reel-$VERSION-x86_64.AppImage
  echo "✓ Renamed AppImage to reel-$VERSION-x86_64.AppImage"
elif ls *.AppImage 1> /dev/null 2>&1; then
  # Fallback: find any AppImage that's not the tools
  for img in *.AppImage; do
    if [[ "$img" != linuxdeploy-*.AppImage ]]; then
      mv "$img" reel-$VERSION-x86_64.AppImage
      echo "✓ Renamed $img to reel-$VERSION-x86_64.AppImage"
      break
    fi
  done
else
  echo "✗ No AppImage found after build"
  ls -la
  exit 1
fi

# Verify the AppImage was created successfully
if [ -f "reel-$VERSION-x86_64.AppImage" ]; then
  echo "✓ AppImage created successfully"
  file "reel-$VERSION-x86_64.AppImage"
  ls -lh "reel-$VERSION-x86_64.AppImage"
else
  echo "✗ Expected AppImage file not found"
  ls -la *.AppImage 2>/dev/null || echo "No AppImage files found"
  exit 1
fi
