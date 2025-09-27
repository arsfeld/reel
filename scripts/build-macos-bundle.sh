#!/bin/bash
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Building macOS app bundle for Reel...${NC}"

# Check if we're on macOS
if [[ "$OSTYPE" != "darwin"* ]]; then
    echo -e "${RED}Error: This script must be run on macOS${NC}"
    exit 1
fi

# All required tools are provided by Nix environment

# Step 1: Generate ICNS from SVG if needed
if [ ! -f "data/macos/icon.icns" ]; then
    echo -e "${YELLOW}Generating app icon...${NC}"

    # Create temporary directory for icon generation
    ICONSET_DIR=$(mktemp -d)/Reel.iconset
    mkdir -p "$ICONSET_DIR"

    # Use rsvg-convert (provided by Nix) to generate different icon sizes
    for size in 16 32 64 128 256 512 1024; do
        rsvg-convert -w $size -h $size data/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg -o "$ICONSET_DIR/icon_${size}x${size}.png"
        if [ $size -ne 1024 ]; then
            # Also create @2x versions for Retina displays
            size2x=$((size * 2))
            rsvg-convert -w $size2x -h $size2x data/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg -o "$ICONSET_DIR/icon_${size}x${size}@2x.png"
        fi
    done

    # Convert iconset to icns
    iconutil -c icns "$ICONSET_DIR" -o data/macos/icon.icns
    echo -e "${GREEN}✓ App icon generated${NC}"
else
    echo -e "${GREEN}✓ App icon already exists${NC}"
fi

# Step 2: Build release binary
echo -e "${YELLOW}Building release binary...${NC}"
cargo build --release
echo -e "${GREEN}✓ Release binary built${NC}"

# Step 3: cargo-bundle is provided by Nix
echo -e "${YELLOW}Using cargo-bundle from Nix environment...${NC}"

# Step 4: Create bundle
echo -e "${YELLOW}Creating app bundle...${NC}"
cargo bundle --release

# Step 5: Post-process the bundle
BUNDLE_PATH="target/release/bundle/osx/Reel.app"

if [ ! -d "$BUNDLE_PATH" ]; then
    echo -e "${RED}Error: Bundle not found at $BUNDLE_PATH${NC}"
    exit 1
fi

echo -e "${YELLOW}Post-processing bundle...${NC}"

# Copy GTK theme files if available
if [ -d "$WHITESUR_GTK_THEME" ]; then
    echo "  Copying WhiteSur GTK theme..."
    mkdir -p "$BUNDLE_PATH/Contents/Resources/share/themes"
    cp -r "$WHITESUR_GTK_THEME/share/themes/WhiteSur"* "$BUNDLE_PATH/Contents/Resources/share/themes/" 2>/dev/null || true
fi

# Create a launcher script that sets up the environment
cat > "$BUNDLE_PATH/Contents/MacOS/reel-launcher" << 'EOF'
#!/bin/bash

# Get the bundle's resource path
BUNDLE_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BUNDLE_RESOURCES="$BUNDLE_DIR/Resources"
BUNDLE_FRAMEWORKS="$BUNDLE_DIR/Frameworks"
BUNDLE_LIB="$BUNDLE_DIR/lib"
BUNDLE_MACOS="$BUNDLE_DIR/MacOS"

# Debug output (can be commented out later)
echo "Bundle paths:" >&2
echo "  BUNDLE_DIR: $BUNDLE_DIR" >&2
echo "  BUNDLE_FRAMEWORKS: $BUNDLE_FRAMEWORKS" >&2
echo "  BUNDLE_LIB: $BUNDLE_LIB" >&2

# Set up GTK environment
export GTK_THEME=WhiteSur-Dark
export GTK_DATA_PREFIX="$BUNDLE_RESOURCES"
export XDG_DATA_DIRS="$BUNDLE_RESOURCES/share:$XDG_DATA_DIRS"
export GTK_EXE_PREFIX="$BUNDLE_RESOURCES"
export GTK_PATH="$BUNDLE_RESOURCES"
export GDK_PIXBUF_MODULE_FILE="$BUNDLE_LIB/gdk-pixbuf-2.0/2.10.0/loaders.cache"
export GDK_PIXBUF_MODULEDIR="$BUNDLE_LIB/gdk-pixbuf-2.0/2.10.0/loaders"

# Set up GStreamer plugins
export GST_PLUGIN_SYSTEM_PATH_1_0="$BUNDLE_LIB/gstreamer-1.0"
export GST_PLUGIN_PATH_1_0="$BUNDLE_LIB/gstreamer-1.0"
export GST_PLUGIN_SCANNER_1_0="$BUNDLE_RESOURCES/libexec/gstreamer-1.0/gst-plugin-scanner"

# Set up library paths - include both Frameworks and lib directories
export DYLD_LIBRARY_PATH="$BUNDLE_FRAMEWORKS:$BUNDLE_LIB:$DYLD_LIBRARY_PATH"
export DYLD_FALLBACK_LIBRARY_PATH="$BUNDLE_FRAMEWORKS:$BUNDLE_LIB:/usr/local/lib:/usr/lib"

# MPV configuration
export MPV_HOME="$BUNDLE_RESOURCES"

# SDL configuration - SDL2 is required by libavdevice but should not initialize
# Prevent SDL2 from initializing since we only need it as a dependency
export SDL_VIDEODRIVER=dummy
export SDL_AUDIODRIVER=dummy
export SDL_RENDER_DRIVER=software
export SDL_INIT_JOYSTICK=0
export SDL_INIT_HAPTIC=0
export SDL_INIT_GAMECONTROLLER=0
export SDL_INIT_EVENTS=0
export SDL_INIT_SENSOR=0
# Disable SDL completely for the app
export SDL_DISABLE=1
# Tell MPV to use native drivers for actual playback
export MPV_VIDEO_OUTPUT=libmpv
export MPV_AUDIO_OUTPUT=coreaudio

# Launch the actual binary with error checking
REEL_BIN="$BUNDLE_MACOS/reel"
if [ ! -f "$REEL_BIN" ]; then
    echo "Error: reel binary not found at $REEL_BIN" >&2
    exit 1
fi

echo "Launching: $REEL_BIN" >&2
exec "$REEL_BIN" "$@"
EOF

chmod +x "$BUNDLE_PATH/Contents/MacOS/reel-launcher"

# Update Info.plist to use the launcher (plutil is system-provided on macOS)
plutil -replace CFBundleExecutable -string "reel-launcher" "$BUNDLE_PATH/Contents/Info.plist"

echo -e "${GREEN}✓ Bundle post-processing complete${NC}"

# Step 6: Bundle dependencies manually since dylibbundler might not catch everything
echo -e "${YELLOW}Bundling dependencies...${NC}"

mkdir -p "$BUNDLE_PATH/Contents/Frameworks"
mkdir -p "$BUNDLE_PATH/Contents/lib"

# First try using dylibbundler for the main binary
echo "Running dylibbundler..."
dylibbundler \
    -od -b \
    -x "$BUNDLE_PATH/Contents/MacOS/reel" \
    -d "$BUNDLE_PATH/Contents/Frameworks" \
    -p @executable_path/../Frameworks/ \
    2>&1 | tee /tmp/dylibbundler.log || echo -e "${YELLOW}Warning: Some libraries could not be bundled${NC}"

# Bundle SDL2 library (required by libavdevice)
echo "Bundling SDL2 library (required by libavdevice)..."

# First check if SDL2 was already bundled by dylibbundler
SDL2_BUNDLED=0
if [ -f "$BUNDLE_PATH/Contents/Frameworks/libSDL2-2.0.0.dylib" ]; then
    echo "  SDL2 already bundled by dylibbundler"
    SDL2_BUNDLED=1
fi

# If not bundled and available from Nix, copy it
if [ "$SDL2_BUNDLED" -eq 0 ] && [ -n "${SDL2:-}" ]; then
    echo "  Copying SDL2 from Nix store..."
    cp "${SDL2}/lib/libSDL2"*.dylib "$BUNDLE_PATH/Contents/Frameworks/" 2>/dev/null || true

    # Fix the library paths
    for lib in "$BUNDLE_PATH/Contents/Frameworks/"libSDL2*.dylib; do
        if [ -f "$lib" ]; then
            echo "  Processing $(basename "$lib")"

            # Strip existing signature
            codesign --remove-signature "$lib" 2>/dev/null || true

            # Fix library ID
            install_name_tool -id "@executable_path/../Frameworks/$(basename "$lib")" "$lib" 2>/dev/null || true

            # Update any SDL dependencies
            otool -L "$lib" | grep -E "(SDL|/usr/local|/opt)" | while read -r dep rest; do
                if [[ "$dep" == /* ]] && [[ "$dep" != /usr/lib/* ]] && [[ "$dep" != /System/* ]]; then
                    newname="@executable_path/../Frameworks/$(basename "$dep")"
                    install_name_tool -change "$dep" "$newname" "$lib" 2>/dev/null || true
                fi
            done

            # Re-sign with ad-hoc signature
            codesign --force --sign - "$lib" 2>/dev/null || true
        fi
    done
    echo "  SDL2 library bundled successfully"
else
    if [ "$SDL2_BUNDLED" -eq 0 ]; then
        echo "  Warning: SDL2 not found in Nix store"
    fi
fi

# Copy GTK and related libraries from Nix store if available
if [ -n "${GTK4:-}" ]; then
    echo "Copying GTK libraries..."
    cp -r "$GTK4/lib/"*.dylib "$BUNDLE_PATH/Contents/Frameworks/" 2>/dev/null || true
fi

# Copy GStreamer plugins if available
if [ -n "${GST_PLUGIN_SYSTEM_PATH_1_0:-}" ]; then
    echo "Copying GStreamer plugins..."
    mkdir -p "$BUNDLE_PATH/Contents/lib/gstreamer-1.0"
    cp -r "$GST_PLUGIN_SYSTEM_PATH_1_0/"*.dylib "$BUNDLE_PATH/Contents/lib/gstreamer-1.0/" 2>/dev/null || true
fi

# Copy MPV library if it exists
if [ -n "${MPV:-}" ]; then
    echo "Copying MPV libraries..."
    cp "$MPV/lib/"*.dylib "$BUNDLE_PATH/Contents/Frameworks/" 2>/dev/null || true
fi

echo -e "${GREEN}✓ Dependencies bundled${NC}"

# Step 7: Sign all libraries and the bundle
echo -e "${YELLOW}Signing app bundle...${NC}"

# Sign all dylibs and frameworks with ad-hoc signature
echo "  Signing frameworks and libraries..."
find "$BUNDLE_PATH/Contents/Frameworks" -type f -name "*.dylib" | while read -r lib; do
    echo "    Signing $(basename "$lib")"
    codesign --remove-signature "$lib" 2>/dev/null || true
    codesign --force --sign - "$lib" 2>/dev/null || true
done

find "$BUNDLE_PATH/Contents/lib" -type f -name "*.dylib" -o -name "*.so" | while read -r lib; do
    echo "    Signing $(basename "$lib")"
    codesign --remove-signature "$lib" 2>/dev/null || true
    codesign --force --sign - "$lib" 2>/dev/null || true
done

# Sign the main binary
echo "  Signing main binary..."
codesign --remove-signature "$BUNDLE_PATH/Contents/MacOS/reel" 2>/dev/null || true
codesign --force --sign - "$BUNDLE_PATH/Contents/MacOS/reel" 2>/dev/null || true

# Sign the launcher script (if executable)
if [ -x "$BUNDLE_PATH/Contents/MacOS/reel-launcher" ]; then
    codesign --force --sign - "$BUNDLE_PATH/Contents/MacOS/reel-launcher" 2>/dev/null || true
fi

# Finally, sign the entire bundle
echo "  Signing complete bundle..."
codesign --force --deep --sign - "$BUNDLE_PATH" 2>/dev/null || true

echo -e "${GREEN}✓ App bundle signed with ad-hoc signature${NC}"

# Step 8: Verify the bundle
echo -e "${YELLOW}Verifying bundle...${NC}"

# Check if the bundle structure is correct
if [ -f "$BUNDLE_PATH/Contents/MacOS/reel" ] && \
   [ -f "$BUNDLE_PATH/Contents/MacOS/reel-launcher" ] && \
   [ -f "$BUNDLE_PATH/Contents/Info.plist" ] && \
   [ -f "$BUNDLE_PATH/Contents/Resources/icon.icns" ]; then
    echo -e "${GREEN}✓ Bundle structure verified${NC}"
else
    echo -e "${RED}✗ Bundle structure incomplete${NC}"
    exit 1
fi

# Display bundle info
echo ""
echo -e "${GREEN}════════════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}✓ macOS app bundle created successfully!${NC}"
echo -e "${GREEN}════════════════════════════════════════════════════════════${NC}"
echo ""
echo "Bundle location: $BUNDLE_PATH"
echo "Bundle size: $(du -sh "$BUNDLE_PATH" | cut -f1)"
echo ""
echo "To run the app:"
echo "  open $BUNDLE_PATH"
echo ""
echo "To install to Applications:"
echo "  cp -r $BUNDLE_PATH /Applications/"
echo ""

# Optional: Create DMG for distribution
if [ "${CREATE_DMG:-0}" = "1" ]; then
    echo -e "${YELLOW}Creating DMG for distribution...${NC}"

    DMG_NAME="Reel-$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2).dmg"

    # Use system hdiutil (always available on macOS)
    hdiutil create -volname "Reel" -srcfolder "$BUNDLE_PATH" -ov -format UDZO "target/release/bundle/$DMG_NAME"
    echo -e "${GREEN}✓ DMG created: target/release/bundle/$DMG_NAME${NC}"
fi