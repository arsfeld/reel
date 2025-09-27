---
id: task-256
title: Add proper bundle signing with Apple Developer account
status: To Do
assignee: []
created_date: '2025-09-26 17:24'
labels:
  - macos
  - packaging
  - security
dependencies: []
priority: high
---

## Description

Implement proper code signing using an Apple Developer ID certificate instead of ad-hoc signing. This will allow the app to run without Gatekeeper warnings and enable distribution outside the App Store.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Configure signing identity in build script (use security find-identity to locate Developer ID)
- [ ] #2 Add entitlements.plist with required app permissions
- [ ] #3 Sign all frameworks and dylibs with Developer ID
- [ ] #4 Sign the main app bundle with hardened runtime enabled
- [ ] #5 Verify signature with codesign --verify and spctl --assess
- [ ] #6 Add optional notarization step for Gatekeeper approval
<!-- AC:END -->
