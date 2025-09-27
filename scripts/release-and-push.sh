#!/usr/bin/env bash
set -e

echo "Reel Release & Push Automation"
echo "==============================="
echo ""

# Run the make-release script
bash scripts/make-release.sh "$@"

# Extract the new version that was just created
NEW_VERSION=$(git describe --tags --abbrev=0)

echo "Pushing release to origin..."
git push origin main
git push origin "$NEW_VERSION"

echo ""
echo "✅ Release $NEW_VERSION successfully pushed!"
echo ""
echo "Next steps:"
echo "  1. Go to GitHub and create a release from tag $NEW_VERSION"
echo "  2. Upload release artifacts (AppImage, .deb, .rpm)"
echo "  3. Update release notes"