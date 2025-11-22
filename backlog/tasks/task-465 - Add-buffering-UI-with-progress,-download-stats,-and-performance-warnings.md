---
id: task-465
title: 'Add buffering UI with progress, download stats, and performance warnings'
status: To Do
assignee: []
created_date: '2025-11-22 18:32'
labels: []
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When media is loading or buffering during playback, users currently see no feedback about what's happening or why it's slow. This creates a poor user experience especially when network conditions are poor or the cache is still downloading chunks.

Users need:
- Visual indication that buffering is happening with progress percentage
- Real-time download speed and data transfer stats
- Clear warnings when buffering can't keep up with playback
- Understanding of whether delays are due to network, disk, or other factors

This will improve the user experience by providing transparency during media loading and helping users understand network/performance issues.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Buffering overlay appears when media is loading or rebuffering
- [ ] #2 Progress bar shows buffering percentage (0-100%)
- [ ] #3 Download speed is displayed in human-readable format (KB/s or MB/s)
- [ ] #4 Total bytes downloaded/cached are shown
- [ ] #5 Warning appears when buffer level is critically low
- [ ] #6 Warning appears when download speed is slower than playback bitrate
- [ ] #7 UI automatically hides when buffering completes and playback resumes
- [ ] #8 Buffering stats update in real-time during download
- [ ] #9 UI follows design patterns from existing player controls
- [ ] #10 Performance warning provides actionable information to user
<!-- AC:END -->
