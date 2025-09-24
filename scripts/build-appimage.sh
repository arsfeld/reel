#!/usr/bin/env bash
set -e

echo "Building AppImage..."

VERSION=${VERSION:-$(grep '^version' Cargo.toml | cut -d'"' -f2)}
ARCH=${ARCH:-x86_64}
export VERSION
export ARCH
export DEPLOY_GTK_VERSION=4
export LINUXDEPLOY_OUTPUT_VERSION=$VERSION

# Map architecture names
if [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then
    APPIMAGE_ARCH="aarch64"
    ARCH_SUFFIX="aarch64"
    LINUXDEPLOY_ARCH="aarch64"
else
    APPIMAGE_ARCH="x86_64"
    ARCH_SUFFIX="x86_64"
    LINUXDEPLOY_ARCH="x86_64"
fi

echo "Building AppImage for version: $VERSION (arch: $APPIMAGE_ARCH)"

# Clean up previous builds
rm -rf AppDir
rm -rf squashfs-root
rm -f *.AppImage
rm -f linuxdeploy-*.AppImage
rm -f linuxdeploy-plugin-*.sh

# Download AppImage tools
echo "=== Setting up AppImage tools ==="
wget -q https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-${LINUXDEPLOY_ARCH}.AppImage
chmod +x linuxdeploy-${LINUXDEPLOY_ARCH}.AppImage

# Extract LinuxDeploy AppImage if FUSE is not available
if ! ./linuxdeploy-${LINUXDEPLOY_ARCH}.AppImage --help &>/dev/null; then
  echo "FUSE not available, extracting LinuxDeploy AppImage..."
  ./linuxdeploy-${LINUXDEPLOY_ARCH}.AppImage --appimage-extract &>/dev/null
  LINUXDEPLOY="./squashfs-root/AppRun"
else
  LINUXDEPLOY="./linuxdeploy-${LINUXDEPLOY_ARCH}.AppImage"
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
if [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then
  # For cross-compilation, check multiple possible locations
  if [ -f "reel-linux-aarch64" ]; then
    # Docker build extracts it with this name
    cp reel-linux-aarch64 AppDir/usr/bin/reel
  elif [ -f "target/aarch64-unknown-linux-gnu/release/reel" ]; then
    cp target/aarch64-unknown-linux-gnu/release/reel AppDir/usr/bin/
  elif [ -f "target/release/reel" ]; then
    # Docker build puts it in standard location
    cp target/release/reel AppDir/usr/bin/
  elif [ -f "reel" ]; then
    # Or it might be extracted to current directory
    cp reel AppDir/usr/bin/
  else
    echo "Error: Binary not found in any expected location"
    echo "Checked: reel-linux-aarch64, target/aarch64-unknown-linux-gnu/release/reel, target/release/reel, ./reel"
    exit 1
  fi
else
  if [ -f "reel-linux-x86_64" ]; then
    # Docker build extracts it with this name
    cp reel-linux-x86_64 AppDir/usr/bin/reel
  elif [ -f "target/release/reel" ]; then
    cp target/release/reel AppDir/usr/bin/
  elif [ -f "reel" ]; then
    cp reel AppDir/usr/bin/
  else
    echo "Error: Binary not found at reel-linux-x86_64, target/release/reel or ./reel"
    exit 1
  fi
fi
chmod +x AppDir/usr/bin/reel
cp data/dev.arsfeld.Reel.desktop AppDir/usr/share/applications/
cp data/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg AppDir/usr/share/icons/hicolor/scalable/apps/

# Set library paths for dependency detection (Ubuntu standard paths)
if [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then
  export LD_LIBRARY_PATH="/usr/lib/aarch64-linux-gnu:$LD_LIBRARY_PATH"
else
  export LD_LIBRARY_PATH="/usr/lib/x86_64-linux-gnu:$LD_LIBRARY_PATH"
fi

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
  mv Reel-*.AppImage reel-$VERSION-$ARCH_SUFFIX.AppImage
  echo "✓ Renamed AppImage to reel-$VERSION-$ARCH_SUFFIX.AppImage"
elif ls *.AppImage 1> /dev/null 2>&1; then
  # Fallback: find any AppImage that's not the tools
  for img in *.AppImage; do
    if [[ "$img" != linuxdeploy-*.AppImage ]]; then
      mv "$img" reel-$VERSION-$ARCH_SUFFIX.AppImage
      echo "✓ Renamed $img to reel-$VERSION-$ARCH_SUFFIX.AppImage"
      break
    fi
  done
else
  echo "✗ No AppImage found after build"
  ls -la
  exit 1
fi

# Verify the AppImage was created successfully
if [ -f "reel-$VERSION-$ARCH_SUFFIX.AppImage" ]; then
  echo "✓ AppImage created successfully"
  file "reel-$VERSION-$ARCH_SUFFIX.AppImage"
  ls -lh "reel-$VERSION-$ARCH_SUFFIX.AppImage"
else
  echo "✗ Expected AppImage file not found"
  ls -la *.AppImage 2>/dev/null || echo "No AppImage files found"
  exit 1
fi
