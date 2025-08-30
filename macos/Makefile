.PHONY: all build run clean release test install help

# Default target
all: build

# Help target
help:
	@echo "Reel macOS Build Targets:"
	@echo ""
	@echo "  make build    - Build the app in Debug mode"
	@echo "  make run      - Build and run the app"
	@echo "  make release  - Build the app in Release mode"
	@echo "  make clean    - Clean all build artifacts"
	@echo "  make test     - Run tests"
	@echo "  make install  - Build and install to /Applications"
	@echo "  make help     - Show this help message"
	@echo ""
	@echo "Environment variables:"
	@echo "  VERBOSE=1     - Show detailed build output"

# Build in Debug mode
build:
	@./build.sh

# Build and run
run:
	@./build.sh --run

# Build in Release mode
release:
	@./build.sh --release

# Clean build artifacts
clean:
	@./build.sh --clean

# Run tests
test: build
	@echo "Running tests..."
	@xcodebuild \
		-project Reel.xcodeproj \
		-scheme Reel \
		-configuration Debug \
		-derivedDataPath build \
		CODE_SIGN_IDENTITY="-" \
		CODE_SIGNING_REQUIRED=NO \
		test

# Install to /Applications
install: release
	@echo "Installing Reel to /Applications..."
	@if [ -d "build/Build/Products/Release/Reel.app" ]; then \
		rm -rf /Applications/Reel.app 2>/dev/null || true; \
		cp -R build/Build/Products/Release/Reel.app /Applications/; \
		echo "✅ Reel installed to /Applications"; \
	else \
		echo "❌ Release build not found. Run 'make release' first."; \
		exit 1; \
	fi

# CI build target (no interactive prompts, fail on error)
ci-build:
	@echo "Building for CI..."
	@./build.sh --release

# Package for distribution
package: release
	@echo "Creating distribution package..."
	@mkdir -p dist
	@if [ -d "build/Build/Products/Release/Reel.app" ]; then \
		cd build/Build/Products/Release && \
		zip -r ../../../../dist/Reel-macOS.zip Reel.app && \
		cd ../../../../ && \
		echo "✅ Package created: dist/Reel-macOS.zip"; \
	else \
		echo "❌ Release build not found"; \
		exit 1; \
	fi