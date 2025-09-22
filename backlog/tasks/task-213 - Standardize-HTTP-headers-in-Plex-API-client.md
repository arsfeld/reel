---
id: task-213
title: Standardize HTTP headers in Plex API client
status: Done
assignee:
  - '@claude'
created_date: '2025-09-22 14:48'
updated_date: '2025-09-22 15:06'
labels:
  - backend
  - plex
  - refactoring
  - tech-debt
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Replace all individual header settings across the Plex API modules with a centralized header management system. Currently, headers like X-Plex-Token, Accept, X-Plex-Client-Identifier, and others are set individually in each API call. This should be abstracted into a standard method that can be easily modified in one place to change headers across all API calls.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Create a centralized header builder method in PlexApi client
- [x] #2 Replace all individual header() calls with the standardized method
- [x] #3 Ensure X-Plex-Token is consistently applied across all requests
- [x] #4 Include all Plex-specific headers (Client-Identifier, Product, Version, Platform) in standard headers
- [x] #5 Support optional headers for specific endpoints while maintaining the standard set
- [x] #6 All existing tests must continue to pass
- [x] #7 Document the header management pattern for future API additions
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Study existing header usage patterns across all Plex API modules
2. Design a centralized header builder method in PlexApi client
3. Implement the header builder with support for standard and optional headers
4. Systematically replace all individual header() calls in each module
5. Update tests to verify header consistency
6. Document the new header management pattern
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Summary

Successfully standardized HTTP headers in the Plex API client by:

### Created Centralized Header Management
- Added public constants for Plex headers (PLEX_PRODUCT, PLEX_VERSION, PLEX_CLIENT_IDENTIFIER, PLEX_PLATFORM) in client.rs
- Implemented `standard_headers()` method that returns a HeaderMap with all standard headers including token and Accept header
- Added `headers_with_extras()` method for cases requiring additional headers

### Updated All API Modules
- **library.rs**: Replaced 5 occurrences of individual header() calls with standard_headers()
- **home.rs**: Updated 3 occurrences including concurrent requests in spawned tasks
- **progress.rs**: Standardized headers for timeline, mark_watched, and mark_unwatched endpoints
- **streaming.rs**: Updated stream URL fetching to use standard headers
- **markers.rs**: Converted from URL-embedded token to using standard headers with query parameters
- **client.rs**: Updated get_machine_id() method to use standard headers

### Maintained Consistency
- Unified PLEX_CLIENT_IDENTIFIER value to "reel-media-player" across all modules
- Exported constants from api module for use in auth.rs
- Updated auth.rs to use shared constants instead of duplicating values

### Testing
- All 12 Plex backend tests passing
- Code compiles successfully with no errors
- Header consistency verified across all API calls

### Documentation
- Added comprehensive documentation explaining the header management pattern
- Included usage examples for both standard and extended header scenarios
- Documented benefits: consistency, maintainability, error reduction, and extensibility

This refactoring ensures all Plex API requests now use consistent headers from a single source of truth, making the codebase more maintainable and less error-prone.
<!-- SECTION:NOTES:END -->
