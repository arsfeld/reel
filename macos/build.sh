#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Reel macOS Build Script${NC}"
echo "========================"

# Parse command line arguments
BUILD_CONFIG="Debug"
ACTION="build"
VERBOSE=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --release)
            BUILD_CONFIG="Release"
            shift
            ;;
        --run)
            ACTION="run"
            shift
            ;;
        --clean)
            ACTION="clean"
            shift
            ;;
        --verbose)
            VERBOSE="-verbose"
            shift
            ;;
        --help)
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --release    Build in Release configuration (default: Debug)"
            echo "  --run        Build and run the application"
            echo "  --clean      Clean build artifacts"
            echo "  --verbose    Show detailed build output"
            echo "  --help       Show this help message"
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
    esac
done

# Get the directory of this script
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

# Function to check and install dependencies
check_dependencies() {
    echo -e "${YELLOW}Checking dependencies...${NC}"
    
    # Check for Xcode Command Line Tools
    if ! xcode-select -p &> /dev/null; then
        echo -e "${RED}Xcode Command Line Tools not found${NC}"
        echo "Installing Xcode Command Line Tools..."
        xcode-select --install
        echo "Please complete the installation and run this script again"
        exit 1
    fi
    
    # Check for XcodeGen
    if ! command -v xcodegen &> /dev/null; then
        echo -e "${YELLOW}XcodeGen not found, installing...${NC}"
        if command -v brew &> /dev/null; then
            brew install xcodegen
        else
            echo -e "${RED}Homebrew not found. Installing Homebrew first...${NC}"
            /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
            brew install xcodegen
        fi
    fi
    
    # Check for Rust
    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}Rust not found${NC}"
        echo "Installing Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    fi
    
    echo -e "${GREEN}All dependencies installed${NC}"
}

# Function to generate Xcode project
generate_project() {
    echo -e "${YELLOW}Generating Xcode project...${NC}"
    
    if [ -d "Reel.xcodeproj" ]; then
        rm -rf Reel.xcodeproj
    fi
    
    xcodegen generate --spec project.yml --quiet
    
    if [ ! -d "Reel.xcodeproj" ]; then
        echo -e "${RED}Failed to generate Xcode project${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}Xcode project generated${NC}"
}

# Function to build Rust library
build_rust() {
    echo -e "${YELLOW}Building Rust library...${NC}"
    
    cd ..
    
    if [ "$BUILD_CONFIG" = "Release" ]; then
        RUST_PROFILE="--release"
    else
        RUST_PROFILE=""
    fi
    
    # Build with or without Nix
    if command -v nix &> /dev/null; then
        echo "Building with Nix environment..."
        nix develop -c cargo build --no-default-features --features macos $RUST_PROFILE
    else
        cargo build --no-default-features --features macos $RUST_PROFILE
    fi
    
    cd "$SCRIPT_DIR"
    echo -e "${GREEN}Rust library built${NC}"
}

# Function to build the app
build_app() {
    echo -e "${YELLOW}Building macOS app (${BUILD_CONFIG})...${NC}"
    
    # Ensure the Rust library is in place
    if [ "$BUILD_CONFIG" = "Release" ]; then
        RUST_LIB="../target/release/libreel_ffi.dylib"
    else
        RUST_LIB="../target/debug/libreel_ffi.dylib"
    fi
    
    if [ ! -f "$RUST_LIB" ]; then
        echo -e "${RED}Rust library not found at $RUST_LIB${NC}"
        echo "Building Rust library first..."
        build_rust
    fi
    
    # Copy the library to the expected location
    cp "$RUST_LIB" "libreel_ffi.dylib"
    
    # Build using xcodebuild
    xcodebuild \
        -project Reel.xcodeproj \
        -scheme Reel \
        -configuration "$BUILD_CONFIG" \
        -derivedDataPath build \
        CODE_SIGN_IDENTITY="-" \
        CODE_SIGNING_REQUIRED=NO \
        CODE_SIGNING_ALLOWED=NO \
        DEVELOPMENT_TEAM="" \
        $VERBOSE \
        build
    
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}Build successful!${NC}"
        
        # Show app location
        APP_PATH="build/Build/Products/${BUILD_CONFIG}/Reel.app"
        if [ -d "$APP_PATH" ]; then
            echo -e "${GREEN}App location: ${SCRIPT_DIR}/${APP_PATH}${NC}"
        fi
    else
        echo -e "${RED}Build failed${NC}"
        exit 1
    fi
}

# Function to run the app
run_app() {
    APP_PATH="build/Build/Products/${BUILD_CONFIG}/Reel.app"
    
    if [ ! -d "$APP_PATH" ]; then
        echo -e "${RED}App not found. Building first...${NC}"
        build_app
    fi
    
    echo -e "${YELLOW}Launching Reel...${NC}"
    open "$APP_PATH"
    
    # Also show logs
    echo -e "${YELLOW}Showing app logs (press Ctrl+C to stop)...${NC}"
    log stream --predicate 'subsystem == "dev.arsfeld.Reel"' --info --debug
}

# Function to clean build artifacts
clean_build() {
    echo -e "${YELLOW}Cleaning build artifacts...${NC}"
    
    rm -rf build
    rm -rf Reel.xcodeproj
    rm -rf Generated
    rm -f libreel_ffi.dylib
    
    # Also clean Rust build
    cd ..
    cargo clean
    cd "$SCRIPT_DIR"
    
    echo -e "${GREEN}Clean complete${NC}"
}

# Main execution
case $ACTION in
    clean)
        clean_build
        ;;
    build)
        check_dependencies
        generate_project
        build_rust
        build_app
        ;;
    run)
        check_dependencies
        generate_project
        build_rust
        build_app
        run_app
        ;;
    *)
        echo -e "${RED}Unknown action: $ACTION${NC}"
        exit 1
        ;;
esac

echo -e "${GREEN}Done!${NC}"