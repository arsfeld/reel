#!/bin/bash

# Create a fancy DMG installer with background image and positioning
# Requires: brew install create-dmg

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
APP_NAME="Reel"
BUNDLE_NAME="${APP_NAME}.app"
VERSION=$(grep '^version' ../Cargo.toml | cut -d'"' -f2)
DMG_NAME="${APP_NAME}-${VERSION}-macOS.dmg"
VOLUME_NAME="${APP_NAME} ${VERSION}"

echo -e "${GREEN}Creating fancy DMG installer for Reel v${VERSION}${NC}"

# Check if app bundle exists
if [ ! -d "$BUNDLE_NAME" ]; then
    echo -e "${RED}Error: App bundle not found${NC}"
    echo "Please run ./create-app-bundle.sh first"
    exit 1
fi

# Check if create-dmg is installed
if ! command -v create-dmg &> /dev/null; then
    echo -e "${YELLOW}Warning: create-dmg not found${NC}"
    echo "Install with: brew install create-dmg"
    echo ""
    echo "Falling back to simple DMG creation..."
    
    # Simple DMG creation
    DMG_TEMP="dmg-temp"
    rm -rf "$DMG_TEMP"
    mkdir -p "$DMG_TEMP"
    cp -r "${BUNDLE_NAME}" "$DMG_TEMP/"
    ln -s /Applications "$DMG_TEMP/Applications"
    rm -f "$DMG_NAME"
    hdiutil create -volname "$VOLUME_NAME" -srcfolder "$DMG_TEMP" -ov -format UDZO "$DMG_NAME"
    rm -rf "$DMG_TEMP"
    
    echo -e "${GREEN}Simple DMG created: $DMG_NAME${NC}"
    exit 0
fi

# Create background image if it doesn't exist
BACKGROUND_DIR="dmg-background"
BACKGROUND_IMG="${BACKGROUND_DIR}/background.png"

if [ ! -f "$BACKGROUND_IMG" ]; then
    echo -e "${GREEN}Creating DMG background image...${NC}"
    mkdir -p "$BACKGROUND_DIR"
    
    # Create a simple background using ImageMagick if available
    if command -v convert &> /dev/null; then
        # Create a gradient background with text
        convert -size 600x400 \
            gradient:'#2e3440'-'#3b4252' \
            -font Helvetica-Bold -pointsize 48 \
            -fill white -gravity north \
            -annotate +0+50 "${APP_NAME}" \
            -font Helvetica -pointsize 18 \
            -annotate +0+120 "Drag to Applications to install" \
            "$BACKGROUND_IMG"
    else
        echo -e "${YELLOW}ImageMagick not found, using default background${NC}"
        # Create a basic colored background using built-in tools
        if command -v sips &> /dev/null; then
            # Create a solid color image
            printf "\x89PNG\r\n\x1a\n" > "$BACKGROUND_IMG"
            # This creates a basic placeholder - in production, provide a proper image
        fi
    fi
fi

# Remove old DMG if it exists
rm -f "$DMG_NAME"
rm -f "${DMG_NAME}.tmp.dmg"

# Create fancy DMG using create-dmg
echo -e "\n${GREEN}Creating fancy DMG with custom layout...${NC}"

create-dmg \
    --volname "$VOLUME_NAME" \
    --volicon "${BUNDLE_NAME}/Contents/Resources/AppIcon.icns" \
    --window-pos 200 120 \
    --window-size 600 400 \
    --icon-size 100 \
    --icon "${APP_NAME}.app" 150 200 \
    --hide-extension "${APP_NAME}.app" \
    --app-drop-link 450 200 \
    --text-size 12 \
    --hdiutil-verbose \
    "$DMG_NAME" \
    "$BUNDLE_NAME"

# Alternative create-dmg with more options (if the above fails)
if [ $? -ne 0 ]; then
    echo -e "${YELLOW}Trying alternative DMG creation method...${NC}"
    
    # Prepare staging directory
    DMG_STAGING="dmg-staging"
    rm -rf "$DMG_STAGING"
    mkdir -p "$DMG_STAGING"
    
    # Copy app
    cp -r "$BUNDLE_NAME" "$DMG_STAGING/"
    
    # Create DMG with basic options
    create-dmg \
        --volname "$VOLUME_NAME" \
        --window-size 600 400 \
        --icon-size 100 \
        --icon "${APP_NAME}.app" 150 200 \
        --app-drop-link 450 200 \
        "$DMG_NAME" \
        "$DMG_STAGING"
    
    # Clean up
    rm -rf "$DMG_STAGING"
fi

# Sign the DMG if certificate is available
echo -e "\n${GREEN}Signing DMG...${NC}"
if security find-identity -p codesigning -v | grep -q "Developer ID Application"; then
    CERT=$(security find-identity -p codesigning -v | grep "Developer ID Application" | head -1 | awk '{print $2}')
    codesign --force --sign "$CERT" "$DMG_NAME"
    echo "DMG signed successfully"
    
    # Verify signature
    codesign --verify --verbose "$DMG_NAME"
else
    echo -e "${YELLOW}Warning: No Developer ID certificate found${NC}"
    echo "DMG will not be signed"
fi

# Notarization reminder
echo -e "\n${GREEN}========================================${NC}"
echo -e "${GREEN}✓ Fancy DMG created successfully!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "DMG: $DMG_NAME"
echo "Size: $(du -h "$DMG_NAME" | cut -f1)"
echo ""
echo "To notarize for distribution:"
echo "  xcrun notarytool submit $DMG_NAME --apple-id your@email.com --team-id TEAMID --wait"
echo "  xcrun stapler staple $DMG_NAME"
echo ""
echo "To test the installer:"
echo "  open $DMG_NAME"