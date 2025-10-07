#!/usr/bin/env bash
set -e

# Script to generate release notes using AI CLI tools
# Tries Claude CLI, Gemini CLI, then Codex in order
# This creates polished, human-readable release notes from git commits

echo "Generating release notes with AI..."
echo ""

# Get the previous tag
PREVIOUS_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "")

# Get the new version (either from arg or current HEAD)
NEW_VERSION="${1:-HEAD}"

# Get commit range
if [ -n "$PREVIOUS_TAG" ]; then
  COMMIT_RANGE="$PREVIOUS_TAG..$NEW_VERSION"
  echo "Analyzing commits from $PREVIOUS_TAG to $NEW_VERSION"
else
  COMMIT_RANGE="HEAD"
  echo "First release - analyzing all commits"
fi

# Get commit log with full details
COMMITS=$(git log --pretty=format:"- %s%n  Author: %an, Date: %ad%n  Full message: %b%n" --date=short "$COMMIT_RANGE")

# Create temporary file for the prompt
TEMP_PROMPT=$(mktemp)

cat > "$TEMP_PROMPT" << 'PROMPT_END'
You are generating release notes for Reel, a modern media player for Plex and Jellyfin.

Below are the git commits since the last release. Please generate polished, user-friendly release notes in this exact format:

## What's New

### Features
- **Feature Name**: Clear description of what it does and why users care
[Only include actual new features, not internal refactoring]

### Bug Fixes
- Fixed [clear description of what was broken and how it's fixed]
[Only include user-visible bug fixes]

### Internal Changes
- [Technical changes, refactoring, code quality improvements]
[Group these under Internal Changes, not features]

## System Requirements
- GTK4 and libadwaita
- GStreamer with common plugins
- 64-bit operating system

Guidelines:
1. Use bold (**text**) for feature/fix names
2. Write in past tense for bug fixes ("Fixed", "Corrected", "Resolved")
3. Write features as user benefits, not implementation details
4. Group related commits together
5. Skip boring commits like "bump version", "update deps" unless significant
6. Be concise but informative
7. If a commit has "BREAKING CHANGE" or "!", note it prominently
8. Focus on what users will notice, not internal code changes
9. Internal/refactoring changes go under "Internal Changes"
10. Keep it professional and clear

Here are the commits:

PROMPT_END

# Append commits to prompt
echo "$COMMITS" >> "$TEMP_PROMPT"

PROMPT_CONTENT=$(cat "$TEMP_PROMPT")
NOTES=""
SUCCESS=false

# Try Claude CLI first
if command -v claude &> /dev/null; then
  echo "Trying Claude CLI..."
  if NOTES=$(claude -p "$PROMPT_CONTENT" 2>&1); then
    echo "✓ Successfully generated notes with Claude CLI"
    SUCCESS=true
  else
    echo "✗ Claude CLI failed, trying next option..."
  fi
fi

# Try Gemini CLI if Claude failed
if [ "$SUCCESS" = false ] && command -v gemini &> /dev/null; then
  echo "Trying Gemini CLI..."
  if NOTES=$(gemini -p "$PROMPT_CONTENT" 2>&1); then
    echo "✓ Successfully generated notes with Gemini CLI"
    SUCCESS=true
  else
    echo "✗ Gemini CLI failed, trying next option..."
  fi
fi

# Try Codex if both Claude and Gemini failed
if [ "$SUCCESS" = false ] && command -v codex &> /dev/null; then
  echo "Trying Codex CLI..."
  if NOTES=$(codex -p "$PROMPT_CONTENT" 2>&1); then
    echo "✓ Successfully generated notes with Codex CLI"
    SUCCESS=true
  else
    echo "✗ Codex CLI failed"
  fi
fi

# Clean up
rm "$TEMP_PROMPT"

# Check if we succeeded with any provider
if [ "$SUCCESS" = false ]; then
  echo ""
  echo "Error: All AI providers failed or are not available"
  echo ""
  echo "Install at least one of:"
  echo "  - Claude CLI: npm install -g @anthropic-ai/claude-cli"
  echo "  - Gemini CLI: npm install -g @google/generative-ai-cli (or appropriate package)"
  echo "  - Codex CLI: (appropriate installation method)"
  echo ""
  exit 1
fi

# Output the notes
echo ""
echo "$NOTES"
