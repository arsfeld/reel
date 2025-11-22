---
id: task-428
title: Rethink and automate release strategy
status: Done
assignee:
  - '@claude'
created_date: '2025-10-07 03:13'
updated_date: '2025-10-07 03:38'
labels:
  - release
  - automation
  - devops
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Current release process is manual and error-prone: bump version in Cargo.toml, run cargo update, create tag, push, then CI creates draft release which requires manual editing. Goal is to fully automate this workflow from version bump to published release with auto-generated release notes.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Implement automated changelog/release notes generation from commits or backlog tasks
- [x] #2 Integrate with CI to automatically publish releases
- [x] #3 Document new release process

- [x] #4 Integrate Claude CLI into release script to auto-generate release notes from commits/tasks
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Analyze current release workflow and identify pain points
2. Design improved release notes generation (consider using Claude CLI or git-cliff/release-please)
3. Modify GitHub workflow to auto-publish stable releases (keep drafts for prereleases)
4. Update release scripts to integrate improved changelog generation
5. Test the workflow end-to-end in a safe way
6. Document the new release process in project docs
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Redesigned release workflow to use Claude CLI for polished release notes and decouple from build process:

**Architecture: Hybrid Local + CI Approach**
- Local: Generate high-quality release notes with Claude CLI
- CI/CD: Focus on building packages and publishing releases
- Decouples note generation (creative, local) from builds (automated, CI)

**1. Claude CLI Integration (AC #1 & #4)**
- Created scripts/generate-release-notes.sh:
  - Uses Claude CLI with detailed prompt matching existing release note format
  - Analyzes commits and generates polished, user-friendly notes
  - Groups changes into Features, Bug Fixes, Internal Changes
  - Handles first releases and incremental releases
- Modified scripts/make-release.sh:
  - Calls generate-release-notes.sh during release creation
  - Stores generated notes in annotated git tag
  - Shows preview before tagging
  - Falls back gracefully if Claude unavailable

**2. Workflow Simplification (AC #2)**
- Modified .github/workflows/release.yml:
  - Removed git-cliff installation and generation logic
  - Reads release notes from tag annotation (git tag -l --format=%(contents))
  - Falls back to simple commit list if no detailed notes
  - Stable releases auto-publish, prereleases stay as drafts
  - Workflow now focuses on builds, not note generation

**3. Documentation (AC #3)**
- Updated RELEASE.md comprehensively:
  - Explains hybrid local+CI architecture
  - Documents Claude CLI requirement
  - Provides troubleshooting for Claude issues
  - Includes manual note editing instructions
  - Enhanced checklists for pre/post release

**Why Claude CLI over git-cliff:**
- Generates human-readable, polished notes (not just formatted commits)
- Matches established release note quality and format
- Intelligently categorizes changes without strict commit conventions
- Can understand context and write user-focused descriptions
- User specifically requested LLM tool, not automated formatter

**Files Modified:**
- scripts/generate-release-notes.sh: Created (new)
- scripts/make-release.sh: Integrated Claude CLI, shows preview
- scripts/release-and-push.sh: Updated messaging
- .github/workflows/release.yml: Simplified to read from tag
- RELEASE.md: Comprehensive update for new workflow
- nix/devshell.nix: No git-cliff dependency needed

**Testing Approach:**
- Test with prerelease tag first (e.g., v0.7.6-test1)
- Verify Claude generates quality notes matching v0.7.5 format
- Confirm tag annotation contains notes
- Verify workflow reads notes correctly
- Test fallback if Claude unavailable

**Update: Added AI Fallbacks**
- scripts/generate-release-notes.sh now tries multiple AI providers:
  1. Claude CLI (preferred, already available in nix shell)
  2. Gemini CLI (fallback)
  3. Codex CLI (fallback)
- Graceful degradation: tries each in order until one succeeds
- If all fail, falls back to simple tag message
- Updated RELEASE.md documentation to cover all AI options
- This ensures release notes generation is resilient even if one provider is unavailable
<!-- SECTION:NOTES:END -->
