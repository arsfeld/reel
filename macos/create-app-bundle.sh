#!/bin/bash

# Reel macOS App Bundle Creator
# This script creates a proper macOS .app bundle from the cargo build

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
APP_NAME="Reel"
BUNDLE_NAME="${APP_NAME}.app"
IDENTIFIER="dev.arsfeld.Reel"
VERSION=$(grep '^version' ../Cargo.toml | cut -d'"' -f2)
BUILD_TYPE=${1:-release}

echo -e "${GREEN}Building Reel macOS App Bundle v${VERSION}${NC}"
echo "Build type: ${BUILD_TYPE}"

# Determine binary path based on build type
if [ "$BUILD_TYPE" = "debug" ]; then
    BINARY_PATH="../target/debug/reel"
    echo -e "${YELLOW}Building debug version (not recommended for distribution)${NC}"
else
    BINARY_PATH="../target/release/reel"
fi

# Build the Rust project
echo -e "\n${GREEN}Step 1: Building Rust project...${NC}"
cd ..
if [ "$BUILD_TYPE" = "debug" ]; then
    cargo build
else
    cargo build --release
fi
cd macos

# Check if binary exists
if [ ! -f "$BINARY_PATH" ]; then
    echo -e "${RED}Error: Binary not found at $BINARY_PATH${NC}"
    echo "Please run 'cargo build --release' first"
    exit 1
fi

# Clean up old bundle
echo -e "\n${GREEN}Step 2: Cleaning up old bundle...${NC}"
rm -rf "$BUNDLE_NAME"

# Create bundle structure
echo -e "\n${GREEN}Step 3: Creating bundle structure...${NC}"
mkdir -p "${BUNDLE_NAME}/Contents/MacOS"
mkdir -p "${BUNDLE_NAME}/Contents/Resources"
mkdir -p "${BUNDLE_NAME}/Contents/Frameworks"
mkdir -p "${BUNDLE_NAME}/Contents/SharedSupport"

# Copy binary
echo -e "\n${GREEN}Step 4: Copying binary...${NC}"
cp "$BINARY_PATH" "${BUNDLE_NAME}/Contents/MacOS/${APP_NAME}"
chmod +x "${BUNDLE_NAME}/Contents/MacOS/${APP_NAME}"

# Copy Info.plist
echo -e "\n${GREEN}Step 5: Copying Info.plist...${NC}"
sed "s/0.1.0/${VERSION}/g" Info.plist > "${BUNDLE_NAME}/Contents/Info.plist"

# Create and copy icon
echo -e "\n${GREEN}Step 6: Processing app icon...${NC}"
if [ -f "../data/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg" ]; then
    # Create iconset directory
    ICONSET_DIR="AppIcon.iconset"
    mkdir -p "$ICONSET_DIR"
    
    # Convert SVG to PNG at various sizes using rsvg-convert if available
    if command -v rsvg-convert &> /dev/null; then
        echo "Generating icon sizes..."
        rsvg-convert -w 16 -h 16 ../data/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg > "$ICONSET_DIR/icon_16x16.png"
        rsvg-convert -w 32 -h 32 ../data/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg > "$ICONSET_DIR/icon_16x16@2x.png"
        rsvg-convert -w 32 -h 32 ../data/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg > "$ICONSET_DIR/icon_32x32.png"
        rsvg-convert -w 64 -h 64 ../data/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg > "$ICONSET_DIR/icon_32x32@2x.png"
        rsvg-convert -w 128 -h 128 ../data/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg > "$ICONSET_DIR/icon_128x128.png"
        rsvg-convert -w 256 -h 256 ../data/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg > "$ICONSET_DIR/icon_128x128@2x.png"
        rsvg-convert -w 256 -h 256 ../data/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg > "$ICONSET_DIR/icon_256x256.png"
        rsvg-convert -w 512 -h 512 ../data/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg > "$ICONSET_DIR/icon_256x256@2x.png"
        rsvg-convert -w 512 -h 512 ../data/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg > "$ICONSET_DIR/icon_512x512.png"
        rsvg-convert -w 1024 -h 1024 ../data/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg > "$ICONSET_DIR/icon_512x512@2x.png"
        
        # Create icns file
        if command -v iconutil &> /dev/null; then
            iconutil -c icns "$ICONSET_DIR" -o "${BUNDLE_NAME}/Contents/Resources/AppIcon.icns"
            echo "Icon created successfully"
        else
            echo -e "${YELLOW}Warning: iconutil not found, skipping icns creation${NC}"
        fi
        
        # Clean up
        rm -rf "$ICONSET_DIR"
    else
        echo -e "${YELLOW}Warning: rsvg-convert not found, skipping icon generation${NC}"
        echo "Install with: brew install librsvg"
    fi
else
    echo -e "${YELLOW}Warning: SVG icon not found${NC}"
fi

# Copy GTK schemas and resources
echo -e "\n${GREEN}Step 7: Copying GTK resources...${NC}"

# Create launcher script that sets up GTK environment
cat > "${BUNDLE_NAME}/Contents/MacOS/${APP_NAME}-launcher" << 'EOF'
#!/bin/bash

# Get the directory where this script is located
DIR="$(cd "$(dirname "$0")" && pwd)"
BUNDLE_DIR="$(dirname "$DIR")"
RESOURCES_DIR="${BUNDLE_DIR}/Resources"
FRAMEWORKS_DIR="${BUNDLE_DIR}/Frameworks"

# Set up GTK environment
export GTK_PATH="${RESOURCES_DIR}/lib/gtk-4.0"
export GTK_DATA_PREFIX="${RESOURCES_DIR}"
export XDG_DATA_DIRS="${RESOURCES_DIR}/share:${XDG_DATA_DIRS}"
export GSETTINGS_SCHEMA_DIR="${RESOURCES_DIR}/share/glib-2.0/schemas"
export GDK_PIXBUF_MODULE_FILE="${RESOURCES_DIR}/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache"
export GDK_PIXBUF_MODULEDIR="${RESOURCES_DIR}/lib/gdk-pixbuf-2.0/2.10.0/loaders"

# Set up GStreamer
export GST_PLUGIN_SYSTEM_PATH_1_0="${RESOURCES_DIR}/lib/gstreamer-1.0"
export GST_PLUGIN_SCANNER="${RESOURCES_DIR}/libexec/gstreamer-1.0/gst-plugin-scanner"

# Set up library paths
export DYLD_LIBRARY_PATH="${FRAMEWORKS_DIR}:${RESOURCES_DIR}/lib:${DYLD_LIBRARY_PATH}"

# Enable high DPI support
export GDK_SCALE=1
export GDK_DPI_SCALE=1

# Force OpenGL backend for better video performance
export GDK_GL="prefer-gl"
export GSK_RENDERER="gl"

# MPV optimizations for macOS
export MPV_COCOA_FORCE_DEDICATED_GPU="1"

# Execute the actual binary
exec "${DIR}/Reel" "$@"
EOF

chmod +x "${BUNDLE_NAME}/Contents/MacOS/${APP_NAME}-launcher"

# Update Info.plist to use launcher
sed -i '' "s/<string>reel<\/string>/<string>${APP_NAME}-launcher<\/string>/" "${BUNDLE_NAME}/Contents/Info.plist"

# Create PkgInfo file
echo -e "\n${GREEN}Step 8: Creating PkgInfo...${NC}"
echo "APPL${IDENTIFIER}" > "${BUNDLE_NAME}/Contents/PkgInfo"

# Copy dylibs (if needed)
echo -e "\n${GREEN}Step 9: Checking for dynamic libraries...${NC}"
if command -v otool &> /dev/null; then
    echo "Analyzing dependencies..."
    DEPS=$(otool -L "${BUNDLE_NAME}/Contents/MacOS/${APP_NAME}" | grep -v /System/ | grep -v /usr/lib/ | awk '{print $1}' | grep -v "^${BUNDLE_NAME}")
    
    if [ ! -z "$DEPS" ]; then
        echo "Copying required libraries..."
        for dep in $DEPS; do
            if [ -f "$dep" ]; then
                cp "$dep" "${BUNDLE_NAME}/Contents/Frameworks/"
                echo "  - $(basename $dep)"
            fi
        done
    else
        echo "No external libraries to copy"
    fi
else
    echo -e "${YELLOW}Warning: otool not found, skipping library analysis${NC}"
fi

# Sign the bundle (if certificate is available)
echo -e "\n${GREEN}Step 10: Code signing...${NC}"
if security find-identity -p codesigning -v | grep -q "Developer ID Application"; then
    CERT=$(security find-identity -p codesigning -v | grep "Developer ID Application" | head -1 | awk '{print $2}')
    echo "Signing with certificate: $CERT"
    
    # Sign frameworks first
    if [ -d "${BUNDLE_NAME}/Contents/Frameworks" ]; then
        find "${BUNDLE_NAME}/Contents/Frameworks" -name "*.dylib" -o -name "*.framework" | while read -r item; do
            codesign --force --deep --sign "$CERT" "$item"
        done
    fi
    
    # Sign the main app
    codesign --force --deep --sign "$CERT" --entitlements entitlements.plist "${BUNDLE_NAME}"
    
    # Verify signature
    codesign --verify --deep --strict "${BUNDLE_NAME}"
    echo "Code signing successful"
else
    echo -e "${YELLOW}Warning: No Developer ID certificate found${NC}"
    echo "The app will not be signed. Users will see security warnings."
    echo "To sign the app, you need a Developer ID certificate from Apple."
fi

# Create a simple DMG for distribution
echo -e "\n${GREEN}Step 11: Creating DMG...${NC}"
DMG_NAME="${APP_NAME}-${VERSION}-macOS.dmg"
VOLUME_NAME="${APP_NAME} ${VERSION}"

# Create temporary directory for DMG contents
DMG_TEMP="dmg-temp"
rm -rf "$DMG_TEMP"
mkdir -p "$DMG_TEMP"

# Copy app bundle
cp -r "${BUNDLE_NAME}" "$DMG_TEMP/"

# Create Applications symlink
ln -s /Applications "$DMG_TEMP/Applications"

# Create DMG
if command -v hdiutil &> /dev/null; then
    # Remove old DMG if exists
    rm -f "$DMG_NAME"
    
    # Create DMG
    hdiutil create -volname "$VOLUME_NAME" -srcfolder "$DMG_TEMP" -ov -format UDZO "$DMG_NAME"
    
    # Clean up
    rm -rf "$DMG_TEMP"
    
    echo -e "${GREEN}DMG created: $DMG_NAME${NC}"
else
    echo -e "${YELLOW}Warning: hdiutil not found, skipping DMG creation${NC}"
    rm -rf "$DMG_TEMP"
fi

# Summary
echo -e "\n${GREEN}========================================${NC}"
echo -e "${GREEN}✓ App bundle created successfully!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "Bundle: ${BUNDLE_NAME}"
echo "Version: ${VERSION}"
echo "Identifier: ${IDENTIFIER}"
if [ -f "$DMG_NAME" ]; then
    echo "DMG: $DMG_NAME"
fi
echo ""
echo "To run the app:"
echo "  open ${BUNDLE_NAME}"
echo ""
echo "To install:"
echo "  cp -r ${BUNDLE_NAME} /Applications/"
echo ""
if [ ! -f "${BUNDLE_NAME}/Contents/Resources/AppIcon.icns" ]; then
    echo -e "${YELLOW}Note: Icon generation was skipped. Install librsvg for icon support:${NC}"
    echo "  brew install librsvg"
fi