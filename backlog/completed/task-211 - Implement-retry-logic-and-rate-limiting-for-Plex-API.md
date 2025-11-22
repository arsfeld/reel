---
id: task-211
title: Implement retry logic and rate limiting for Plex API
status: Done
assignee:
  - '@claude-code'
created_date: '2025-09-22 14:19'
updated_date: '2025-10-06 00:09'
labels:
  - backend
  - plex
  - api
  - error-handling
dependencies:
  - task-206
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add robust error handling, retry logic, and rate limiting awareness to the Plex API implementation. This will improve reliability and handle transient network issues and API rate limits gracefully.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Exponential backoff retry logic handles transient HTTP failures
- [x] #2 Rate limiting detection and handling prevents API blocking
- [x] #3 Detailed error parsing extracts specific error codes from API responses
- [x] #4 Typed error enums differentiate between failure modes (auth, network, rate limit, server)
- [x] #5 Request/response logging aids in debugging API issues
- [x] #6 Retry logic respects maximum attempt limits and timeouts
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Create typed error enum (PlexApiError) to differentiate failure modes:
   - Authentication errors (401, 403)
   - Network errors (connection, timeout)
   - Rate limiting (429)
   - Server errors (500+)
   - Client errors (400-499)

2. Design retry strategy with exponential backoff:
   - Implement RetryPolicy struct with configurable max attempts and base delay
   - Only retry on transient failures (network, 500+, 429)
   - Don't retry on permanent failures (4xx auth/client errors)

3. Create request wrapper with retry logic:
   - Wrap reqwest operations with retry_with_backoff() helper
   - Add rate limit detection from status codes and headers
   - Implement exponential backoff calculation

4. Add request/response logging:
   - Log all API requests with URL and method
   - Log response status, headers, and body for errors
   - Use tracing for structured logging

5. Update PlexApi client:
   - Add RetryPolicy configuration to PlexApi
   - Replace direct client calls with retry wrapper
   - Ensure timeout respects total retry time

6. Test retry logic:
   - Manual testing with network interruptions
   - Verify exponential backoff timing
   - Check max attempts are respected
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Summary

Implemented comprehensive retry logic and rate limiting for the Plex API to improve reliability and handle transient network failures gracefully.

## Changes

### New Files
- **src/backends/plex/api/errors.rs**: Typed error enum (`PlexApiError`) that differentiates between:
  - Authentication errors (401, 403) - permanent, non-retryable
  - Rate limiting (429) - retryable with server-specified backoff
  - Server errors (500+) - transient, retryable
  - Client errors (400-499) - permanent, non-retryable
  - Network errors (timeout, connection) - transient, retryable

- **src/backends/plex/api/retry.rs**: Exponential backoff retry infrastructure with:
  - `RetryPolicy` struct with configurable max attempts, delays, and timeouts
  - Smart retry logic that only retries transient failures
  - Exponential backoff calculation: `min(base_delay * 2^attempt, max_delay)`
  - Support for server-specified retry-after headers
  - Comprehensive unit tests (5 tests, all passing)

### Modified Files
- **src/backends/plex/api/mod.rs**: Export new error and retry modules
- **src/backends/plex/api/client.rs**: 
  - Added `retry_policy` field to `PlexApi`
  - Implemented `execute_get()` helper that wraps HTTP requests with:
    - Automatic retry with exponential backoff
    - Detailed request/response logging (debug and warn levels)
    - Rate limit detection from status codes
    - Typed error handling
  - Updated `get_machine_id()` to use retry logic

- **src/backends/plex/api/library.rs**:
  - Updated `get_libraries()` to use retry logic
  - Updated `get_movies()` to use retry logic

## Configuration

Default retry policy:
- Max attempts: 3 retries (4 total attempts)
- Base delay: 100ms
- Max delay: 10 seconds
- Total timeout: 30 seconds

Can be customized via `PlexApi::with_retry_policy()`.

## Testing

All unit tests pass (5/5):
- Exponential backoff delay calculation
- Successful retry after transient failures
- No retry on permanent errors (auth failures)
- Respects max attempt limits
- Delay capping at max_delay

## Migration Path

Existing code continues to work without changes. The retry logic is automatically applied to all API calls using `execute_get()`. No breaking changes to public API.
<!-- SECTION:NOTES:END -->
