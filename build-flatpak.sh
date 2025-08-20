#!/bin/bash
set -e

echo "Building Reel Flatpak..."

# Check for required tools
if ! command -v flatpak-builder &> /dev/null; then
    echo "Error: flatpak-builder is not installed"
    echo "Install it with: flatpak install flathub org.flatpak.Builder"
    exit 1
fi

if ! command -v python3 &> /dev/null; then
    echo "Error: python3 is not installed"
    exit 1
fi

# Download flatpak-cargo-generator if not present
if [ ! -f "flatpak-cargo-generator.py" ]; then
    echo "Downloading flatpak-cargo-generator.py..."
    curl -L -o flatpak-cargo-generator.py \
        https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py
    chmod +x flatpak-cargo-generator.py
fi

# Generate cargo sources
echo "Generating cargo-sources.json..."
python3 flatpak-cargo-generator.py ./Cargo.lock -o cargo-sources.json

# Build the Flatpak
echo "Building Flatpak..."
flatpak-builder --user --install --force-clean build-dir dev.arsfeld.Reel.json

echo "Build complete! You can run the app with:"
echo "  flatpak run dev.arsfeld.Reel"