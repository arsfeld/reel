# macOS Build and Distribution

This directory contains all the necessary files and scripts to build Reel as a native macOS application bundle.

## Prerequisites

1. **Xcode Command Line Tools** (required for code signing):
   ```bash
   xcode-select --install
   ```

2. **Homebrew packages** (optional but recommended):
   ```bash
   brew install librsvg    # For icon generation
   brew install create-dmg  # For prettier DMG creation
   ```

3. **Developer ID Certificate** (optional, for distribution):
   - Required for signing the app to avoid security warnings
   - Get from Apple Developer Program ($99/year)
   - Install in Keychain Access

## Building the App Bundle

### Quick Build

```bash
cd macos
chmod +x create-app-bundle.sh
./create-app-bundle.sh
```

This will:
1. Build the Rust project in release mode
2. Create a proper .app bundle structure
3. Generate app icons from the SVG source
4. Set up GTK/GStreamer environment
5. Sign the app (if certificate available)
6. Create a DMG for distribution

### Debug Build

```bash
./create-app-bundle.sh debug
```

## Files

- `Info.plist` - macOS app metadata and configuration
- `entitlements.plist` - Security entitlements for code signing
- `create-app-bundle.sh` - Main build script
- `create-dmg-installer.sh` - Create fancy DMG with background (optional)

## Distribution Options

### 1. Direct .app Bundle
Simply distribute the `Reel.app` directory. Users can drag it to Applications.

### 2. DMG Installer
The build script automatically creates a simple DMG. For a fancier DMG with custom background:
```bash
./create-dmg-installer.sh
```

### 3. Homebrew Cask
Create a cask formula for easy installation via Homebrew:
```ruby
cask "reel" do
  version "0.1.0"
  sha256 "..."
  url "https://github.com/arsfeld/reel/releases/download/v#{version}/Reel-#{version}-macOS.dmg"
  name "Reel"
  desc "Modern media player for Plex and Jellyfin"
  homepage "https://github.com/arsfeld/reel"
  
  app "Reel.app"
end
```

### 4. Mac App Store (Future)
Requires additional work:
- Sandbox compliance
- App Store Connect account
- Review process

## Code Signing

### Ad-hoc Signing (no certificate required)
```bash
codesign --force --deep -s - Reel.app
```

### Developer ID Signing (recommended)
The build script automatically signs if a certificate is found.

To manually sign:
```bash
codesign --force --deep --sign "Developer ID Application: Your Name" --entitlements entitlements.plist Reel.app
```

### Notarization (required for distribution)
After signing, submit for notarization:
```bash
xcrun notarytool submit Reel-0.1.0-macOS.dmg --apple-id your@email.com --team-id TEAMID --wait
xcrun stapler staple Reel.app
```

## Architecture Notes

### Current Implementation (GTK on macOS)
- Uses GTK4 through homebrew/MacPorts
- Runs in XQuartz or native Quartz backend
- Full feature parity with Linux version
- Some UI elements may feel non-native

### Future Native Implementation
Plans for a native macOS frontend:
- SwiftUI/AppKit UI (see `../src/platforms/macos/` - TODO)
- Native menu bar and dock integration
- Touch Bar support
- System media key integration
- Continuity features

### Hybrid Approach (Recommended Short-term)
Keep GTK for main UI but add native macOS features:
- Native menu bar using objc crate
- Dock badge updates
- System notifications
- Media key handling

## Troubleshooting

### App won't open ("damaged" error)
- Right-click and select "Open" to bypass Gatekeeper
- Or remove quarantine: `xattr -cr Reel.app`

### Missing libraries
- Ensure GTK4 is installed: `brew install gtk4 libadwaita`
- Check dependencies: `otool -L Reel.app/Contents/MacOS/Reel`

### Icon not showing
- Install librsvg: `brew install librsvg`
- Rebuild the bundle

### Performance issues
- Ensure Metal acceleration is enabled
- Check Activity Monitor for GPU usage
- Try switching between MPV and GStreamer backends

## Integration with Nix Flake

The Nix flake includes macOS support. In the development shell:
```bash
nix develop
cargo build --release
cd macos && ./create-app-bundle.sh
```

## Testing

Test the app bundle:
```bash
# Run directly
./Reel.app/Contents/MacOS/Reel-launcher

# Open as app
open Reel.app

# Check signature
codesign --verify --verbose Reel.app
spctl --assess --verbose Reel.app
```

## Known Limitations

1. **GTK on macOS**: Some UI elements don't follow macOS design guidelines
2. **Menu Bar**: Currently using GTK menus instead of native macOS menu bar
3. **Fullscreen**: May not work perfectly with macOS spaces
4. **Touch Bar**: Not yet implemented
5. **Handoff/Continuity**: Not yet supported

## Future Enhancements

- [ ] Native SwiftUI frontend
- [ ] Mac Catalyst version for M1 iPads
- [ ] iCloud sync for watch progress
- [ ] SharePlay integration
- [ ] Shortcuts app actions
- [ ] Quick Look previews
- [ ] Spotlight integration