#!/usr/bin/env bash
set -e

echo "Reel Release Automation"
echo "======================="
echo ""

# Check for clean working directory
if [ -n "$(git status --porcelain)" ]; then
  echo "Error: Working directory is not clean. Please commit or stash changes."
  exit 1
fi

# Get current version from Cargo.toml
CURRENT_VERSION=$(grep '^version = ' Cargo.toml | head -1 | cut -d'"' -f2)
echo "Current version: $CURRENT_VERSION"

# Parse version components
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"

# Determine version bump type (default: patch)
BUMP_TYPE="${1:-patch}"

case "$BUMP_TYPE" in
  major)
    NEW_MAJOR=$((MAJOR + 1))
    NEW_VERSION="$NEW_MAJOR.0.0"
    ;;
  minor)
    NEW_MINOR=$((MINOR + 1))
    NEW_VERSION="$MAJOR.$NEW_MINOR.0"
    ;;
  patch)
    NEW_PATCH=$((PATCH + 1))
    NEW_VERSION="$MAJOR.$MINOR.$NEW_PATCH"
    ;;
  *)
    echo "Error: Invalid bump type. Use 'major', 'minor', or 'patch'."
    exit 1
    ;;
esac

echo "New version: $NEW_VERSION"
echo ""

# Update version in Cargo.toml
echo "Updating Cargo.toml..."
sed -i.bak "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" Cargo.toml
rm Cargo.toml.bak

# Update Cargo.lock
echo "Updating Cargo.lock..."
cargo update --package reel

# Run tests to ensure everything still works
echo "Running tests..."
cargo test

# Build to ensure compilation
echo "Building project..."
cargo build --release

# Commit version bump
echo "Creating release commit..."
git add Cargo.toml Cargo.lock
git commit -m "chore: release v$NEW_VERSION"

# Create annotated tag
echo "Creating release tag..."
git tag -a "v$NEW_VERSION" -m "Release v$NEW_VERSION"

# Show what will be pushed
echo ""
echo "Ready to push the following:"
echo "  - Commit: chore: release v$NEW_VERSION"
echo "  - Tag: v$NEW_VERSION"
echo ""
echo "To push the release, run:"
echo "  git push origin main"
echo "  git push origin v$NEW_VERSION"
echo ""
echo "Or push everything at once with:"
echo "  git push origin main --tags"