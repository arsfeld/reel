#!/usr/bin/env bash
set -e

# Test script to validate built packages work in Ubuntu and Fedora containers
# This script tests the binary, package installations, and AppImage

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Testing Reel packages in containers ===${NC}"
echo ""

# Get version from Cargo.toml
VERSION=$(grep '^version' "$PROJECT_ROOT/Cargo.toml" | cut -d'"' -f2)
echo "Version: $VERSION"

# Build test images
echo -e "${YELLOW}Building test container images...${NC}"
docker build -t reel-test-ubuntu:24.04 -f "$SCRIPT_DIR/Dockerfile.test-ubuntu" "$SCRIPT_DIR" > /dev/null 2>&1
echo -e "${GREEN}✓ Ubuntu test image built${NC}"
docker build -t reel-test-fedora:40 -f "$SCRIPT_DIR/Dockerfile.test-fedora" "$SCRIPT_DIR" > /dev/null 2>&1
echo -e "${GREEN}✓ Fedora test image built${NC}"
echo ""

# Check if packages exist
echo -e "${YELLOW}Checking for built packages...${NC}"
PACKAGES_FOUND=true

if [ ! -f "$PROJECT_ROOT/reel-linux-x86_64" ]; then
    echo -e "${RED}✗ Binary not found: reel-linux-x86_64${NC}"
    PACKAGES_FOUND=false
else
    echo -e "${GREEN}✓ Binary found${NC}"
fi

if [ ! -f "$PROJECT_ROOT/reel-$VERSION-amd64.deb" ]; then
    echo -e "${RED}✗ Debian package not found: reel-$VERSION-amd64.deb${NC}"
    PACKAGES_FOUND=false
else
    echo -e "${GREEN}✓ Debian package found${NC}"
fi

if [ ! -f "$PROJECT_ROOT/reel-$VERSION-x86_64.rpm" ]; then
    echo -e "${RED}✗ RPM package not found: reel-$VERSION-x86_64.rpm${NC}"
    PACKAGES_FOUND=false
else
    echo -e "${GREEN}✓ RPM package found${NC}"
fi

if [ ! -f "$PROJECT_ROOT/reel-$VERSION-x86_64.AppImage" ]; then
    echo -e "${RED}✗ AppImage not found: reel-$VERSION-x86_64.AppImage${NC}"
    PACKAGES_FOUND=false
else
    echo -e "${GREEN}✓ AppImage found${NC}"
fi

if [ "$PACKAGES_FOUND" = false ]; then
    echo ""
    echo -e "${RED}Some packages are missing. Please build them first with:${NC}"
    echo "./scripts/build-with-docker.sh x86_64 all"
    exit 1
fi

echo ""
echo -e "${GREEN}=== Testing in Ubuntu 24.04 ===${NC}"
echo ""

# Test Ubuntu with binary
echo -e "${YELLOW}1. Testing binary in Ubuntu...${NC}"
if docker run --rm \
    -v "$PROJECT_ROOT/reel-linux-x86_64:/tmp/reel" \
    reel-test-ubuntu:24.04 \
    bash -c "
        chmod +x /tmp/reel
        /tmp/reel --version 2>&1 | grep -q 'Starting Reel application' && echo '✓ Binary starts in Ubuntu'
    "; then
    echo -e "${GREEN}✓ Binary test passed${NC}"
else
    echo -e "${RED}✗ Binary failed in Ubuntu${NC}"
fi

# Test Ubuntu with .deb package
echo ""
echo -e "${YELLOW}2. Testing .deb package installation in Ubuntu...${NC}"
if docker run --rm \
    -v "$PROJECT_ROOT/reel-$VERSION-amd64.deb:/tmp/reel.deb" \
    reel-test-ubuntu:24.04 \
    bash -c "
        # Install the package
        dpkg -i /tmp/reel.deb 2>/dev/null || apt-get install -f -y > /dev/null 2>&1
        # Check if package is installed correctly
        if dpkg -l | grep -q '^ii.*reel'; then
            echo '✓ Debian package installed successfully'
            if reel --version 2>&1 | grep -q 'Starting Reel application'; then
                echo '✓ Installed binary starts correctly'
                exit 0
            else
                echo '✗ Installed binary failed to start'
                exit 1
            fi
        else
            echo '✗ Package installation issues detected'
            dpkg -l | grep reel
            exit 1
        fi
    "; then
    echo -e "${GREEN}✓ .deb package test passed${NC}"
else
    echo -e "${RED}✗ .deb package failed in Ubuntu${NC}"
fi

# Test Ubuntu with AppImage
echo ""
echo -e "${YELLOW}3. Testing AppImage in Ubuntu...${NC}"
if docker run --rm \
    -v "$PROJECT_ROOT/reel-$VERSION-x86_64.AppImage:/tmp/reel.AppImage" \
    reel-test-ubuntu:24.04 \
    bash -c "
        chmod +x /tmp/reel.AppImage
        # Check if it's a valid AppImage
        if file /tmp/reel.AppImage | grep -q 'ELF.*executable'; then
            echo '✓ AppImage is valid executable format'
            # Try to run with --appimage-extract-and-run (works without FUSE)
            if /tmp/reel.AppImage --appimage-extract-and-run --version 2>&1 | grep -q 'Starting Reel application'; then
                echo '✓ AppImage starts correctly'
                exit 0
            else
                echo '  Note: AppImage needs FUSE to run normally in containers'
                exit 1
            fi
        else
            echo '✗ AppImage is not valid'
            file /tmp/reel.AppImage
            exit 1
        fi
    "; then
    echo -e "${GREEN}✓ AppImage test passed${NC}"
else
    echo -e "${RED}✗ AppImage failed in Ubuntu${NC}"
fi

echo ""
echo -e "${GREEN}=== Testing in Fedora 40 ===${NC}"
echo ""

# Test Fedora with binary
echo -e "${YELLOW}4. Testing binary in Fedora...${NC}"
if docker run --rm \
    -v "$PROJECT_ROOT/reel-linux-x86_64:/tmp/reel" \
    reel-test-fedora:40 \
    bash -c "
        chmod +x /tmp/reel
        /tmp/reel --version 2>&1 | grep -q 'Starting Reel application' && echo '✓ Binary starts in Fedora'
    "; then
    echo -e "${GREEN}✓ Binary test passed${NC}"
else
    echo -e "${RED}✗ Binary failed in Fedora${NC}"
fi

# Test Fedora with .rpm package
echo ""
echo -e "${YELLOW}5. Testing .rpm package installation in Fedora...${NC}"
if docker run --rm \
    -v "$PROJECT_ROOT/reel-$VERSION-x86_64.rpm:/tmp/reel.rpm" \
    reel-test-fedora:40 \
    bash -c "
        # Install the package
        dnf install -y /tmp/reel.rpm > /dev/null 2>&1
        # Check if package is installed correctly
        if rpm -qa | grep -q '^reel-'; then
            echo '✓ RPM package installed successfully'
            if reel --version 2>&1 | grep -q 'Starting Reel application'; then
                echo '✓ Installed binary starts correctly'
                exit 0
            else
                echo '✗ Installed binary failed to start'
                exit 1
            fi
        else
            echo '✗ Package installation issues detected'
            rpm -qa | grep reel
            exit 1
        fi
    "; then
    echo -e "${GREEN}✓ .rpm package test passed${NC}"
else
    echo -e "${RED}✗ .rpm package failed in Fedora${NC}"
fi

# Test Fedora with AppImage
echo ""
echo -e "${YELLOW}6. Testing AppImage in Fedora...${NC}"
if docker run --rm \
    -v "$PROJECT_ROOT/reel-$VERSION-x86_64.AppImage:/tmp/reel.AppImage" \
    reel-test-fedora:40 \
    bash -c "
        chmod +x /tmp/reel.AppImage
        # Check if it's a valid AppImage
        if file /tmp/reel.AppImage | grep -q 'ELF.*executable'; then
            echo '✓ AppImage is valid executable format'
            # Try to run with --appimage-extract-and-run (works without FUSE)
            if /tmp/reel.AppImage --appimage-extract-and-run --version 2>&1 | grep -q 'Starting Reel application'; then
                echo '✓ AppImage starts correctly'
                exit 0
            else
                echo '  Note: AppImage needs FUSE to run normally in containers'
                exit 1
            fi
        else
            echo '✗ AppImage is not valid'
            file /tmp/reel.AppImage
            exit 1
        fi
    "; then
    echo -e "${GREEN}✓ AppImage test passed${NC}"
else
    echo -e "${RED}✗ AppImage failed in Fedora${NC}"
fi

echo ""
echo -e "${GREEN}=== Test Summary ===${NC}"
echo ""
echo "Package validation completed!"
echo ""
echo -e "${YELLOW}Note:${NC} Full GUI functionality requires a display server (X11/Wayland)."
echo "      These tests verify package structure and basic execution."
echo ""
echo "For full runtime testing, install on a system with a complete desktop environment."