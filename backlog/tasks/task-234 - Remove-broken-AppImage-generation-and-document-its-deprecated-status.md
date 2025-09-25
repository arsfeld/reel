---
id: task-234
title: Remove broken AppImage generation and document its deprecated status
status: To Do
assignee: []
created_date: '2025-09-24 18:40'
labels:
  - build
  - documentation
  - cleanup
dependencies: []
priority: medium
---

## Description

Remove AppImage generation from the build system since it's currently broken. Update documentation to reflect this change and add a banner explaining that AppImage styles are fully broken and contributions to fix it are welcome.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Remove AppImage build commands from build scripts
- [ ] #2 Remove AppImage references from README.md
- [ ] #3 Remove AppImage mentions from release notes/changelog
- [ ] #4 Add banner to README explaining AppImage is broken and accepting contributions
- [ ] #5 Remove AppImage build configuration from Nix flake
- [ ] #6 Remove build-appimage command from development environment
- [ ] #7 Update CI/CD workflows to remove AppImage build steps
- [ ] #8 Add issue template for AppImage contribution volunteers
<!-- AC:END -->
