# Release Process

This document describes the automated release process for Reel.

## Overview

Reel uses a hybrid release workflow that:
- **Locally**: Generates polished release notes with AI (Claude CLI, Gemini CLI, or Codex)
- **CI/CD**: Builds packages for multiple platforms and automatically publishes
- Publishes stable releases automatically
- Keeps prereleases as drafts for manual review

This decouples release note generation (local, high-quality with AI) from package building (CI, automated).

## Prerequisites

- Clean git working directory (no uncommitted changes)
- All tests passing
- Access to push to the main branch and create tags
- Nix development environment (`nix develop`)
- At least one AI CLI installed:
  - Claude CLI (already available in nix shell via alias) - preferred
  - Gemini CLI - fallback option
  - Codex CLI - fallback option

## Release Types

Reel follows [Semantic Versioning](https://semver.org/):
- **Major** (X.0.0): Breaking changes, incompatible API changes
- **Minor** (0.X.0): New features, backwards-compatible
- **Patch** (0.0.X): Bug fixes, backwards-compatible

Prerelease suffixes (`-alpha`, `-beta`, `-rc`, `-dev`, `-pre`) will:
- Mark the release as a prerelease on GitHub
- Keep the release as a DRAFT for manual review and publishing
- Example: `v0.8.0-beta1`

## Creating a Release

### Quick Release (Recommended)

For a patch release (most common):

```bash
# Enter nix development shell if not already in it
nix develop

# Create and push release in one command
./scripts/release-and-push.sh patch
```

For minor or major releases:

```bash
./scripts/release-and-push.sh minor  # For new features
./scripts/release-and-push.sh major  # For breaking changes
```

### Step-by-Step Release

If you want more control:

```bash
# Step 1: Create release (bumps version, tests, creates tag)
./scripts/make-release.sh patch  # or 'minor' or 'major'

# This will:
# - Update version in Cargo.toml
# - Update Cargo.lock
# - Run tests
# - Build the project
# - Create a commit with "chore: release vX.Y.Z"
# - Create a git tag "vX.Y.Z"
# - Show a preview of the changelog

# Step 2: Review the changelog and tag

# Step 3: Push when ready
git push origin main
git push origin vX.Y.Z
```

## What Happens After Pushing

Once you push a version tag (e.g., `v0.8.0`), GitHub Actions automatically:

1. **Detects the release type**
   - Stable releases (e.g., `v0.8.0`): Will be published automatically
   - Prereleases (e.g., `v0.8.0-beta1`): Created as DRAFT for manual review

2. **Extracts release notes**
   - Reads the polished notes from the annotated tag (generated locally with AI)
   - Falls back to simple commit list if tag has no detailed notes

3. **Builds packages** (in parallel for speed)
   - Linux x86_64: `.deb`, `.rpm`, standalone binary
   - Linux ARM64: `.deb`, `.rpm`, standalone binary

4. **Publishes release**
   - Stable: Immediately published and visible with the generated notes
   - Prerelease: Saved as draft, review and publish manually

5. **Attaches artifacts**
   - All built packages are automatically uploaded to the release

## Commit Message Format

The AI intelligently processes your commits to generate release notes, but you can help it by:

**Best Practice**: Use clear, descriptive commit messages that explain WHAT changed and WHY.

**Optional**: Follow [Conventional Commits](https://www.conventionalcommits.org/) for consistent categorization:

```
<type>(<scope>): <description>

[optional body explaining why]
```

### Recommended Types

- `feat`: New feature
- `fix`: Bug fix
- `perf`: Performance improvement
- `refactor`: Code refactoring
- `docs`: Documentation
- `test`: Tests
- `build`: Build system
- `ci`: CI/CD changes
- `chore`: Maintenance tasks

### Examples

```bash
# Clear descriptive messages (will be categorized by Claude)
git commit -m "Add playlist shuffle mode"
git commit -m "Fix duplicate library entries during sync"

# Or use conventional commits for consistency
git commit -m "feat(player): add playlist shuffle mode"
git commit -m "fix(sync): prevent duplicate library entries"

# Breaking changes
git commit -m "feat(api)!: redesign authentication flow

BREAKING CHANGE: Auth tokens now expire after 24 hours"
```

**Note**: The AI will intelligently categorize commits even without conventional commit prefixes, but using them helps ensure consistent grouping.

## Monitoring Release

After pushing:

1. Visit https://github.com/arsfeld/reel/actions
2. Find the "Release" workflow for your tag
3. Monitor the build progress
4. Download artifacts if needed
5. For prereleases: Review the draft release and publish when ready

## Manual Release Notes Generation

The `make-release.sh` script automatically generates release notes with AI, but you can also generate them manually:

```bash
# Generate notes for commits since last tag
./scripts/generate-release-notes.sh

# Generate notes for specific version
./scripts/generate-release-notes.sh v0.8.0

# Preview what will be generated before running make-release.sh
./scripts/generate-release-notes.sh HEAD
```

The script tries Claude CLI first, then falls back to Gemini CLI or Codex if needed. It sends your commits with instructions to generate polished, user-friendly release notes matching the established format.

## Troubleshooting

### Release workflow failed

1. Check the Actions tab for error details
2. Common issues:
   - Build failures: Fix and create a new patch release
   - Missing dependencies: Update workflow dependencies
   - Package upload failures: Usually transient, re-run workflow

### Need to fix a release

If you need to update a published release:

1. Delete the tag locally and remotely:
   ```bash
   git tag -d vX.Y.Z
   git push origin :refs/tags/vX.Y.Z
   ```

2. Delete the release on GitHub (Settings → Releases)

3. Fix the issues and create the release again

### Release notes look wrong

If AI generates poor release notes:

1. **Check commit quality**: Write clearer, more descriptive commit messages
2. **Edit manually**: After generation, you can edit the tag annotation before pushing:
   ```bash
   git tag -f -a v0.8.0  # Re-opens editor to edit the notes
   ```
3. **Regenerate**: Delete the tag and run `make-release.sh` again with better commits
4. **Adjust prompt**: Edit `scripts/generate-release-notes.sh` to refine the AI prompt

### AI CLI not available

The script tries AI providers in this order: Claude → Gemini → Codex

If all fail:

1. Check you're in nix development shell: `nix develop`
2. Verify at least one is available:
   ```bash
   which claude  # Should show the alias
   which gemini
   which codex
   ```
3. Test your AI CLI:
   ```bash
   echo "test" | claude -p "Say hello"
   # or
   echo "test" | gemini -p "Say hello"
   ```
4. Install at least one:
   ```bash
   npm install -g @anthropic-ai/claude-cli  # Claude (recommended)
   # or appropriate package for Gemini/Codex
   ```
5. If all fail, the script will fall back to a simple tag message

## Configuration Files

- `scripts/generate-release-notes.sh`: AI integration for release notes (Claude/Gemini/Codex)
- `.github/workflows/release.yml`: GitHub Actions release workflow (reads from tag)
- `scripts/make-release.sh`: Local release creation script (calls AI)
- `scripts/release-and-push.sh`: Automated release and push script

## Release Checklist

Before creating a release:

- [ ] All tests passing (`cargo test`)
- [ ] No uncommitted changes (`git status`)
- [ ] Version bump type decided (major/minor/patch)
- [ ] Reviewed recent commits - are they clear and descriptive?
- [ ] In nix development shell (`nix develop`)
- [ ] At least one AI CLI working (try: `claude -p "test"` or `gemini -p "test"`)

After generating release (but before pushing):

- [ ] Review generated release notes in terminal
- [ ] Edit tag annotation if needed (`git tag -f -a vX.Y.Z`)
- [ ] Notes match the established format and quality

After pushing release:

- [ ] GitHub Actions workflow succeeded
- [ ] Release notes appear correctly on GitHub
- [ ] All packages built successfully
- [ ] For prereleases: Review and publish draft when ready

## See Also

- [Semantic Versioning](https://semver.org/)
- [Conventional Commits](https://www.conventionalcommits.org/) (optional but helpful)
- [Claude CLI Documentation](https://docs.anthropic.com/claude/docs/cli)
