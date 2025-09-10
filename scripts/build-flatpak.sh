#!/usr/bin/env bash
set -e

echo "Building Reel Flatpak..."

VERSION=${VERSION:-$(grep '^version' Cargo.toml | cut -d'"' -f2)}
ARCH=${ARCH:-x86_64}
export VERSION
export ARCH

# Map architecture names
if [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then
    FLATPAK_ARCH="aarch64"
    ARCH_SUFFIX="aarch64"
else
    FLATPAK_ARCH="x86_64"
    ARCH_SUFFIX="x86_64"
fi

echo "Building Flatpak for version: $VERSION (arch: $FLATPAK_ARCH)"

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

# Set up uv for Python dependency management
echo "=== Setting up Python dependency management ==="
if ! command -v uv &> /dev/null; then
    echo "Installing uv..."
    curl -LsSf https://astral.sh/uv/install.sh | sh
    export PATH="$HOME/.cargo/bin:$PATH"
fi
PYTHON_RUN="uv run --with tomlkit --with aiohttp python3"

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
$PYTHON_RUN flatpak-cargo-generator.py ./Cargo.lock -o cargo-sources.json
echo "✓ Generated cargo-sources.json"

# Build the Flatpak with repository
echo "=== Building Flatpak ==="
flatpak-builder --force-clean --sandbox --user --install-deps-from=flathub --arch=$FLATPAK_ARCH --repo=repo build-dir dev.arsfeld.Reel.json

echo "=== Exporting Flatpak bundle ==="
flatpak build-bundle repo "reel-$VERSION-$ARCH_SUFFIX.flatpak" dev.arsfeld.Reel

# Create repository metadata
echo "=== Creating repository metadata ==="
ostree --repo=repo summary -u

echo "=== Build Summary ==="
echo "✓ Flatpak bundle: reel-$VERSION-$ARCH_SUFFIX.flatpak"
echo "✓ Repository: repo/ directory"

if [ -f "reel-$VERSION-$ARCH_SUFFIX.flatpak" ]; then
    echo "✓ Flatpak bundle created successfully"
    file "reel-$VERSION-$ARCH_SUFFIX.flatpak"
    ls -lh "reel-$VERSION-$ARCH_SUFFIX.flatpak"
else
    echo "✗ Expected Flatpak bundle not found"
    exit 1
fi

echo ""
echo "Build complete! You can:"
echo "  1. Install locally: flatpak install --user reel-$VERSION-$ARCH_SUFFIX.flatpak"
echo "  2. Run the app: flatpak run dev.arsfeld.Reel"
echo "  3. Test in sandbox: flatpak run --devel dev.arsfeld.Reel"
