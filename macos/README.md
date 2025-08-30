# Reel macOS Application

This is the native macOS frontend for Reel, built with SwiftUI and integrated with the Rust core via swift-bridge.

## Quick Start

```bash
# Build the app
make build

# Build and run
make run

# Build release version
make release

# Clean build artifacts
make clean
```

## Automated Build System

The build system is fully automated with no manual steps required:

- **Automatic dependency installation** (XcodeGen, Rust if needed)
- **Automatic Xcode project generation** from YAML configuration
- **Integrated Rust library compilation**
- **Swift bridge code generation**
- **Ad-hoc code signing** (no developer account required)
- **One-command build and run**

## Build Options

### Using Make (Recommended)

```bash
make build      # Build Debug version
make run        # Build and run
make release    # Build Release version
make clean      # Clean all artifacts
make test       # Run tests
make install    # Install to /Applications
make package    # Create distribution ZIP
```

### Using Build Script

```bash
./build.sh              # Build Debug version
./build.sh --run        # Build and run
./build.sh --release    # Build Release version
./build.sh --clean      # Clean all artifacts
./build.sh --verbose    # Show detailed output
```

### From Project Root

```bash
./build-macos.sh        # Build from project root
./build-macos.sh --run  # Build and run from project root
```

## CI/CD

The project includes GitHub Actions workflow for automated builds:

- Triggered on push to `main`, `master`, or `macos` branches
- Builds Release version
- Runs tests
- Creates distribution package
- Uploads artifacts
- Creates releases on tags

## Development Workflow

1. **Make changes** to Swift or Rust code
2. **Run** `make run` to test changes
3. **Commit** changes
4. **CI** automatically builds and tests

## Project Structure

```
macos/
├── project.yml           # XcodeGen configuration
├── generate_project.sh   # Script to generate Xcode project
├── Reel/                # Swift source code
│   ├── ReelApp.swift    # Main app entry point
│   ├── ContentView.swift # Main UI views
│   ├── AppModel.swift   # App state and Rust integration
│   ├── Info.plist       # App metadata
│   └── Assets.xcassets/ # App icons and images
└── Generated/           # Swift bridge generated files (auto-created)
```

## Architecture

The macOS app follows a SwiftUI + Rust architecture:

- **SwiftUI Views**: Native macOS UI
- **AppModel**: ObservableObject that manages app state
- **Swift-Bridge**: FFI layer for Rust-Swift communication
- **Rust Core**: Shared backend logic with GTK frontend

## Development Workflow

1. **UI Development**: Edit Swift files in Xcode with live preview
2. **Rust Changes**: Edit Rust code, rebuild with cargo
3. **Bridge Changes**: Modify `bridge.rs`, regenerate with build script
4. **Testing**: Run from Xcode with ⌘R

## Troubleshooting

### XcodeGen not found
```bash
brew install xcodegen
```

### Rust library not building
Ensure you're in the Nix development shell:
```bash
nix develop
```

### Swift bridge files not generated
The build script automatically generates these, but you can manually trigger:
```bash
cd ..
cargo build --no-default-features --features swift
```

### App crashes on launch
Check that the Rust dylib is properly embedded:
```bash
otool -L Reel.app/Contents/MacOS/Reel
```

## Next Steps

The current implementation provides:
- Basic SwiftUI app structure
- XcodeGen-based project management
- Rust library build integration
- Mock data for UI development

TODO:
- [ ] Fix swift-bridge integration
- [ ] Connect to real Rust backend
- [ ] Implement event subscription
- [ ] Add media playback with AVPlayer
- [ ] Implement settings and authentication