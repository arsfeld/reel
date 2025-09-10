#!/usr/bin/env bash
set -e

echo "Building Reel Flatpak..."

VERSION=${VERSION:-$(grep '^version' Cargo.toml | cut -d'"' -f2)}
export VERSION

echo "Building Flatpak for version: $VERSION"

# Check for required tools
if ! command -v flatpak-builder &> /dev/null; then
    echo "Error: flatpak-builder is not installed"
    echo "Install it with your package manager:"
    echo "  Ubuntu/Debian: sudo apt install flatpak-builder"
    echo "  Fedora: sudo dnf install flatpak-builder"
    echo "  Arch: sudo pacman -S flatpak-builder"
    exit 1
fi

if ! command -v python3 &> /dev/null; then
    echo "Error: python3 is not installed"
    exit 1
fi

# Check if required runtimes are installed
echo "=== Checking Flatpak runtimes ==="
if ! flatpak info org.gnome.Platform//48 &> /dev/null; then
    echo "Installing GNOME Platform 48..."
    flatpak install --user flathub org.gnome.Platform//48 -y
fi

if ! flatpak info org.gnome.Sdk//48 &> /dev/null; then
    echo "Installing GNOME SDK 48..."
    flatpak install --user flathub org.gnome.Sdk//48 -y
fi

if ! flatpak info org.freedesktop.Sdk.Extension.rust-stable//23.08 &> /dev/null; then
    echo "Installing Rust SDK extension..."
    flatpak install --user flathub org.freedesktop.Sdk.Extension.rust-stable//23.08 -y
fi

# Clean up previous builds
echo "=== Cleaning up previous builds ==="
rm -rf build-dir
rm -rf .flatpak-builder
rm -rf repo
rm -f *.flatpak

# Download flatpak-cargo-generator if not present
if [ ! -f "flatpak-cargo-generator.py" ]; then
    echo "=== Downloading flatpak-cargo-generator.py ==="
    curl -L -o flatpak-cargo-generator.py \
        https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py
    chmod +x flatpak-cargo-generator.py
fi

# Generate cargo sources
echo "=== Generating cargo-sources.json ==="
python3 flatpak-cargo-generator.py ./Cargo.lock -o cargo-sources.json
echo "✓ Generated cargo-sources.json"

# Build the Flatpak with repository
echo "=== Building Flatpak ==="
flatpak-builder --force-clean --sandbox --user --install-deps-from=flathub --arch=x86_64 --repo=repo build-dir dev.arsfeld.Reel.json

echo "=== Exporting Flatpak bundle ==="
flatpak build-bundle repo "reel-$VERSION-x86_64.flatpak" dev.arsfeld.Reel

# Create repository metadata
echo "=== Creating repository metadata ==="
ostree --repo=repo summary -u

echo "=== Build Summary ==="
echo "✓ Flatpak bundle: reel-$VERSION-x86_64.flatpak"
echo "✓ Repository: repo/ directory"

if [ -f "reel-$VERSION-x86_64.flatpak" ]; then
    echo "✓ Flatpak bundle created successfully"
    file "reel-$VERSION-x86_64.flatpak"
    ls -lh "reel-$VERSION-x86_64.flatpak"
else
    echo "✗ Expected Flatpak bundle not found"
    exit 1
fi

echo ""
echo "Build complete! You can:"
echo "  1. Install locally: flatpak install --user reel-$VERSION-x86_64.flatpak"
echo "  2. Run the app: flatpak run dev.arsfeld.Reel"
echo "  3. Test in sandbox: flatpak run --devel dev.arsfeld.Reel"