#!/bin/bash

set -e

echo "Generating Xcode project with XcodeGen..."

# Check if XcodeGen is installed
if ! command -v xcodegen &> /dev/null; then
    echo "XcodeGen is not installed. Installing via Homebrew..."
    if ! command -v brew &> /dev/null; then
        echo "Error: Homebrew is not installed. Please install Homebrew first."
        echo "Visit: https://brew.sh"
        exit 1
    fi
    brew install xcodegen
fi

# Navigate to the macos directory
cd "$(dirname "$0")"

# Clean up old Xcode project if it exists
if [ -d "Reel.xcodeproj" ]; then
    echo "Removing old Xcode project..."
    rm -rf Reel.xcodeproj
fi

# Generate the new project
echo "Running XcodeGen..."
xcodegen generate

echo "Xcode project generated successfully!"
echo ""
echo "Next steps:"
echo "1. Open Reel.xcodeproj in Xcode"
echo "2. Select your development team in Signing & Capabilities"
echo "3. Build and run the project"
echo ""
echo "To build the Rust library first:"
echo "  cd .. && nix develop -c cargo build --no-default-features --features macos"