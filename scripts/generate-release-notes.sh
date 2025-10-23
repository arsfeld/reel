#!/usr/bin/env bash
set -e

# Script to generate release notes using AI CLI tools
# Tries Claude CLI, Gemini CLI, then Codex in order
# This creates polished, human-readable release notes from git commits

echo "Generating release notes with AI..." >&2
echo "" >&2

# Get the previous tag
PREVIOUS_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "")

# Get the new version (either from arg or current HEAD)
NEW_VERSION="${1:-HEAD}"

# Get commit range
if [ -n "$PREVIOUS_TAG" ]; then
  COMMIT_RANGE="$PREVIOUS_TAG..$NEW_VERSION"
  echo "Analyzing commits from $PREVIOUS_TAG to $NEW_VERSION" >&2
else
  COMMIT_RANGE="HEAD"
  echo "First release - analyzing all commits" >&2
fi

# Get commit log with full details
COMMITS=$(git log --pretty=format:"- %s%n  Author: %an, Date: %ad%n  Full message: %b%n" --date=short "$COMMIT_RANGE")

# Create temporary file for the prompt
TEMP_PROMPT=$(mktemp)

cat > "$TEMP_PROMPT" << 'PROMPT_END'
You are generating release notes for Reel, a modern media player for Plex and Jellyfin.

Below are the git commits since the last release. Generate polished, user-friendly release notes.

IMPORTANT: Output ONLY the release notes in the exact format below. Do NOT include any preamble, introduction, or conversational text. Start directly with "## What's New".

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
  echo "Trying Claude CLI..." >&2
  if NOTES=$(claude -p "$PROMPT_CONTENT" 2>/dev/null); then
    echo "✓ Successfully generated notes with Claude CLI" >&2
    SUCCESS=true
  else
    echo "✗ Claude CLI failed, trying next option..." >&2
  fi
fi

# Try Gemini CLI if Claude failed
if [ "$SUCCESS" = false ] && command -v gemini &> /dev/null; then
  echo "Trying Gemini CLI..." >&2
  if NOTES=$(gemini -p "$PROMPT_CONTENT" 2>/dev/null); then
    echo "✓ Successfully generated notes with Gemini CLI" >&2
    SUCCESS=true
  else
    echo "✗ Gemini CLI failed, trying next option..." >&2
  fi
fi

# Try Codex if both Claude and Gemini failed
if [ "$SUCCESS" = false ] && command -v codex &> /dev/null; then
  echo "Trying Codex CLI..." >&2
  if NOTES=$(codex -p "$PROMPT_CONTENT" 2>/dev/null); then
    echo "✓ Successfully generated notes with Codex CLI" >&2
    SUCCESS=true
  else
    echo "✗ Codex CLI failed" >&2
  fi
fi

# Clean up
rm "$TEMP_PROMPT"

# Check if we succeeded with any provider
if [ "$SUCCESS" = false ]; then
  echo "" >&2
  echo "Error: All AI providers failed or are not available" >&2
  echo "" >&2
  echo "Install at least one of:" >&2
  echo "  - Claude CLI: npm install -g @anthropic-ai/claude-cli" >&2
  echo "  - Gemini CLI: npm install -g @google/generative-ai-cli (or appropriate package)" >&2
  echo "  - Codex CLI: (appropriate installation method)" >&2
  echo "" >&2
  exit 1
fi

# Output the notes (to stdout)
echo "$NOTES"
