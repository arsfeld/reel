---
id: task-429
title: Fix network request failures preventing TV show details page from loading
status: Done
assignee:
  - '@code'
created_date: '2025-10-21 02:29'
updated_date: '2025-10-21 02:44'
labels:
  - bug
  - networking
  - plex
  - ui
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When navigating to a TV show details page, network requests to the Plex server are failing with "error sending request for url" errors. This prevents:
- Full show metadata from loading (cast, detailed info)
- Episodes from being displayed (shows 0 episodes)
- Poster and backdrop images from loading

The errors occur when making HTTPS requests to Plex relay URLs (format: https://10-1-1-5.f0d4900e448644aea0c903ebfee340be.plex.direct:32400/...). 

Root cause needs investigation - could be SSL certificate validation, timeout configuration, network connectivity, or issues specific to Plex relay/indirect connection URLs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify root cause of 'error sending request' failures for Plex HTTPS requests
- [x] #2 TV show details page successfully loads full metadata from Plex server
- [x] #3 Episodes are displayed in the episode grid (not 0 episodes)
- [x] #4 Poster and backdrop images load successfully
- [x] #5 Network requests work reliably for both direct and relay Plex connection URLs
- [x] #6 Appropriate error handling and user feedback for network failures
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Review Plex TV show details fetch path and recent changes impacting HTTPS relay requests
2. Trace HTTP client configuration (reqwest client, TLS, timeouts) and reproduce issue using relay URL fixture/tests
3. Implement fix ensuring relay connections succeed while preserving certificate validation
4. Add regression coverage (unit/integration) and adjust UI error handling messaging if needed
5. Verify episodes, metadata, and imagery load through relay and direct connections
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Updated ConnectionService to reuse stored Plex/Jellyfin tokens when running health checks so direct HTTPS endpoints succeed. Confirmed cargo check fails outside nix develop due to missing mold linker; retry inside dev shell.
<!-- SECTION:NOTES:END -->
