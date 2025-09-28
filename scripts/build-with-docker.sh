#!/usr/bin/env bash
set -e

# Script to build Reel using Docker buildx for multiple architectures
# This uses the shared Dockerfile.build for consistent builds

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DOCKERFILE="$SCRIPT_DIR/Dockerfile.build"

# Parse arguments
ARCH=${1:-$(uname -m)}
shift || true
BUILD_TYPES=${@:-all} # Can pass multiple types: deb rpm binary

# Map architecture names
case "$ARCH" in
    x86_64|amd64)
        DOCKER_PLATFORM="linux/amd64"
        ARCH_NAME="x86_64"
        DEB_ARCH="amd64"
        ;;
    aarch64|arm64)
        DOCKER_PLATFORM="linux/arm64"
        ARCH_NAME="aarch64"
        DEB_ARCH="arm64"
        ;;
    *)
        echo "Unsupported architecture: $ARCH"
        echo "Supported: x86_64, amd64, aarch64, arm64"
        exit 1
        ;;
esac

echo "Building Reel for $ARCH_NAME using Docker buildx"
echo "Build types: $BUILD_TYPES"
echo "Docker platform: $DOCKER_PLATFORM"

# Ensure buildx is available and set up
if ! docker buildx version &>/dev/null; then
    echo "Docker buildx is not available. Please install Docker Desktop or enable buildx."
    exit 1
fi

# Create or use existing buildx builder
BUILDER_NAME="reel-builder"
if ! docker buildx inspect $BUILDER_NAME &>/dev/null; then
    echo "Creating buildx builder: $BUILDER_NAME"
    docker buildx create --name $BUILDER_NAME --use --driver docker-container
else
    echo "Using existing buildx builder: $BUILDER_NAME"
    docker buildx use $BUILDER_NAME
fi

# Build the Docker image for the target architecture
IMAGE_TAG="reel-build:$ARCH_NAME"
echo "Building Docker image: $IMAGE_TAG"

cd "$PROJECT_ROOT"

# Build the image with cargo-chef caching
# Use cache options if provided via environment variables
CACHE_OPTS=""
if [ -n "$BUILDX_CACHE_FROM" ]; then
    CACHE_OPTS="--cache-from $BUILDX_CACHE_FROM"
fi
if [ -n "$BUILDX_CACHE_TO" ]; then
    CACHE_OPTS="$CACHE_OPTS --cache-to $BUILDX_CACHE_TO"
fi

docker buildx build \
    --platform "$DOCKER_PLATFORM" \
    --target builder \
    --tag "$IMAGE_TAG" \
    --load \
    $CACHE_OPTS \
    -f "$DOCKERFILE" \
    .

# Extract version from Cargo.toml
VERSION=$(grep '^version' Cargo.toml | cut -d'"' -f2)
echo "Version: $VERSION"

# Function to run commands in the container
run_in_container() {
    docker run --rm \
        -v "$PROJECT_ROOT:/host" \
        --platform "$DOCKER_PLATFORM" \
        "$IMAGE_TAG" \
        bash -c "$1"
}

# Process each build type
for BUILD_TYPE in $BUILD_TYPES; do
    case "$BUILD_TYPE" in
        all)
            # Build everything
            for TYPE in binary deb rpm; do
                BUILD_TYPES="$TYPE"
                $0 "$ARCH_NAME" "$TYPE" || exit 1
            done
            break  # Don't process other types if 'all' was specified
            ;;

        binary|tarball)
            echo "=== Extracting release binary ==="
            run_in_container "cp /workspace/target/release/reel /host/reel-linux-$ARCH_NAME && \
                             chmod +x /host/reel-linux-$ARCH_NAME"

            # Create tarball if requested
            if [ "$BUILD_TYPE" = "tarball" ]; then
                echo "Creating tarball..."
                tar -czf "reel-linux-$ARCH_NAME.tar.gz" "reel-linux-$ARCH_NAME"
                rm "reel-linux-$ARCH_NAME"  # Remove the raw binary, keep only tarball
                echo "✓ Tarball: reel-linux-$ARCH_NAME.tar.gz"
            else
                echo "✓ Binary: reel-linux-$ARCH_NAME"
            fi
            ;;

        deb)
            echo "=== Building Debian package ==="
            run_in_container "cd /workspace && cargo deb --no-build && \
                             cp target/debian/*.deb /host/reel-$VERSION-$DEB_ARCH.deb"
            echo "✓ Debian package: reel-$VERSION-$DEB_ARCH.deb"
            ;;

        rpm)
            echo "=== Building RPM package ==="
            run_in_container "cd /workspace && cargo generate-rpm && \
                             cp target/generate-rpm/*.rpm /host/reel-$VERSION-$ARCH_NAME.rpm"
            echo "✓ RPM package: reel-$VERSION-$ARCH_NAME.rpm"
            ;;

        appimage)
            echo "=== Building AppImage ==="
            # Set environment variables for the AppImage script
            run_in_container "cd /workspace && \
                             export VERSION=$VERSION && \
                             export ARCH=$ARCH_NAME && \
                             ./scripts/build-appimage.sh && \
                             cp reel-*.AppImage /host/"
            echo "✓ AppImage: reel-$VERSION-$ARCH_NAME.AppImage"
            ;;

        *)
            echo "Unknown build type: $BUILD_TYPE"
            echo "Supported: all, binary, tarball, deb, rpm, appimage"
            exit 1
            ;;
    esac
done

echo ""
echo "=== Build complete ==="
echo "Architecture: $ARCH_NAME"
echo "Built artifacts:"
[ -f "reel-linux-$ARCH_NAME" ] && echo "  - reel-linux-$ARCH_NAME" || true
[ -f "reel-linux-$ARCH_NAME.tar.gz" ] && echo "  - reel-linux-$ARCH_NAME.tar.gz" || true
[ -f "reel-$VERSION-$DEB_ARCH.deb" ] && echo "  - reel-$VERSION-$DEB_ARCH.deb" || true
[ -f "reel-$VERSION-$ARCH_NAME.rpm" ] && echo "  - reel-$VERSION-$ARCH_NAME.rpm" || true
[ -f "reel-$VERSION-$ARCH_NAME.AppImage" ] && echo "  - reel-$VERSION-$ARCH_NAME.AppImage" || true