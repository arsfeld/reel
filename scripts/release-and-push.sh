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
echo "Automated release workflow triggered!"
echo "  - GitHub Actions will build packages for Linux (x86_64 and ARM64)"
echo "  - Release notes will be auto-generated from commits"
if [[ "$NEW_VERSION" =~ -(alpha|beta|rc|dev|pre) ]]; then
  echo "  - Release will be marked as DRAFT (prerelease detected)"
  echo "  - Review and publish manually when ready"
else
  echo "  - Release will be PUBLISHED automatically"
fi
echo ""
echo "Monitor progress at:"
echo "  https://github.com/arsfeld/reel/actions"