---
title: "refactor: Replace Plex XML parsing with JSON"
type: refactor
status: active
date: 2026-03-16
---

# refactor: Replace Plex XML parsing with JSON

## Overview

Migrate the Plex API integration from XML to JSON parsing. The Plex API supports JSON responses via the `Accept: application/json` header. Zig's `std.json` provides built-in typed deserialization, eliminating the need for the homegrown XML parser (171 lines in `xml.zig`). This also fixes XML entity decoding bugs (e.g., `&amp;` appearing in titles) for free.

## Problem Statement / Motivation

The current homegrown XML parser (`src/net/plex/xml.zig`) has several limitations:
- **No XML entity decoding** -- `&amp;`, `&lt;`, `&quot;` appear as raw text in movie titles and summaries
- **No single-quoted attribute support**
- **No CDATA handling**
- **No text content extraction** (only attributes)
- **Custom code to maintain** -- 171 lines of parser + manual attribute extraction at every call site

The Plex API natively supports JSON (`Accept: application/json`), and Zig's `std.json.parseFromSlice` can deserialize directly into typed structs with zero boilerplate. The TMDB client already uses `std.json` (dynamic `Value` extraction), so this aligns with existing codebase patterns.

## Proposed Solution

1. Change `Accept: application/xml` to `Accept: application/json` in `PlexHeaders.toHeaders()`
2. Add JSON response structs for each Plex endpoint shape
3. Replace XML parsing in `client.zig` and `auth.zig` with `std.json` dynamic Value extraction (matching the TMDB pattern)
4. Delete `xml.zig` and remove its import from `lib.zig`

### Parsing Approach: Dynamic `std.json.Value` extraction

Use `std.json.parseFromSlice(std.json.Value, ...)` then manually walk the JSON object tree -- the same pattern used by the TMDB client (`src/net/tmdb/client.zig`). This is preferred over typed struct deserialization because:

- **Resilient** -- `orelse continue` / `orelse null` patterns match current XML code's graceful handling of missing/malformed fields
- **Consistent** -- matches the existing TMDB client pattern in this codebase
- **No field naming issues** -- avoids camelCase struct fields or `@"fieldName"` syntax
- **Partial failure tolerance** -- one bad item in a list doesn't fail the entire parse

### Memory Ownership: Dupe-and-free (unchanged)

Keep the current pattern: parse JSON, `.dupe()` every needed string into the provided allocator, `defer parsed.deinit()` to free the JSON tree. The existing `freeMediaItems()` and manual `allocator.free()` calls in `lib.zig` remain unchanged.

## Technical Considerations

### JSON Response Shapes by Endpoint

| Endpoint | Host | JSON Shape | Array Key |
|----------|------|-----------|-----------|
| `POST /api/v2/pins` | plex.tv | Bare object `{...}` | N/A |
| `GET /api/v2/pins/{id}` | plex.tv | Bare object `{...}` | N/A |
| `GET /api/v2/resources` | plex.tv | Bare array `[{...}]` | Top-level array |
| `GET /library/sections` | PMS | `{MediaContainer: {Directory: [...]}}` | `Directory` |
| `GET /library/sections/{id}/all` | PMS | `{MediaContainer: {Metadata: [...]}}` | `Metadata` |
| `GET /library/onDeck` | PMS | `{MediaContainer: {Metadata: [...]}}` | `Metadata` |
| `GET /library/recentlyAdded` | PMS | `{MediaContainer: {Metadata: [...]}}` | `Metadata` |
| `GET /library/metadata/{id}` | PMS | `{MediaContainer: {Metadata: [...]}}` | `Metadata` |
| `GET /library/metadata/{id}/children` | PMS | `{MediaContainer: {Metadata: [...]}}` | `Metadata` |
| `GET /search?query=...` | PMS | `{MediaContainer: {Metadata: [...]}}` | `Metadata` |

### JSON Field Name Mapping

Plex JSON uses camelCase for scalar fields and PascalCase for nested arrays. Field names are identical to the XML attribute names. Key mappings for `Metadata` items:

| JSON Field | Current XML attr | Type | PlexMediaItem field |
|-----------|-----------------|------|-------------------|
| `ratingKey` | `ratingKey` | string | `rating_key` |
| `title` | `title` | string | `title` |
| `type` | `type` | string | `media_type` |
| `summary` | `summary` | ?string | `summary` |
| `year` | `year` | ?int | `year` |
| `rating` | `rating` | ?float | `rating` |
| `duration` | `duration` | ?int (ms) | `duration_ms` |
| `thumb` | `thumb` | ?string | `thumb` |
| `art` | `art` | ?string | `art` |
| `parentRatingKey` | `parentRatingKey` | ?string | `parent_rating_key` |
| `grandparentRatingKey` | `grandparentRatingKey` | ?string | `grandparent_rating_key` |
| `grandparentTitle` | `grandparentTitle` | ?string | `grandparent_title` |
| `parentIndex` | `parentIndex` | ?int | `parent_index` |
| `index` | `index` | ?int | `index` |
| `viewOffset` | `viewOffset` | ?int | `view_offset` |
| `key` | `key` | ?string | `part_key` |

For server discovery (`resources` endpoint):

| JSON Field | Type | PlexServer field |
|-----------|------|-----------------|
| `name` | string | `name` |
| `clientIdentifier` | string | `machine_identifier` |
| `accessToken` | ?string | `access_token` |
| `provides` | string (comma-separated) | filter for "server" |
| `connections[].uri` | string | `Connection.uri` |
| `connections[].local` | bool | `Connection.local` |
| `connections[].relay` | bool | `Connection.relay` |
| `connections[].protocol` | string | `Connection.protocol` |

For PIN auth:

| JSON Field | Type | PlexAuth field |
|-----------|------|---------------|
| `id` | int | `pin_id` |
| `code` | string | `pin_code` |
| `authToken` | ?string | `auth_token` |

### Key Gotchas

1. **`connections[].local` and `connections[].relay`** are proper JSON booleans (`true`/`false`), not `"0"`/`"1"` strings like in XML. The Value extraction code must use `.object.get("local")` -> check for `.true`/`.false` instead of string comparison.

2. **`provides` is a comma-separated string** (e.g., `"server,client"`), same as XML. Continue using `std.mem.indexOf(u8, provides, "server")`.

3. **`ratingKey` is a string** in JSON (e.g., `"123"`), not an integer. Keep as `[]const u8`.

4. **`rating` can be float or integer** (e.g., `7.5` or `0`). Handle both `.float` and `.integer` cases, same as TMDB's `vote_average` pattern.

5. **Empty containers** -- when a library is empty, `Metadata` key may be absent entirely. Use `obj.get("Metadata") orelse return &.{}`.

6. **String lifetime** -- `response.body` is freed via `defer response.deinit()`. All strings extracted from `parsed.value` reference the arena. Must `.dupe()` every string before `parsed.deinit()`.

## Acceptance Criteria

- [x] `Accept: application/xml` changed to `Accept: application/json` in `types.zig:27`
- [x] `auth.zig:requestPin()` parses JSON PIN response (bare object)
- [x] `auth.zig:pollPin()` parses JSON PIN response (bare object)
- [x] `client.zig:discoverServers()` parses JSON resources response (bare array with nested connections)
- [x] `client.zig:getLibraries()` parses JSON MediaContainer with Directory array
- [x] `client.zig:fetchMediaItems()` parses JSON MediaContainer with Metadata array
- [x] `xml.zig` deleted
- [x] `lib.zig` line 17 (`pub const plex_xml = ...`) removed
- [x] `xml` import removed from `client.zig` and `auth.zig`
- [x] All existing tests pass
- [x] New JSON parsing tests cover each endpoint shape
- [x] `freeMediaItems()` and manual free paths in `lib.zig` remain compatible (no memory leaks or use-after-free)
- [x] Entity-encoded characters in titles/summaries now display correctly (e.g., `&amp;` -> `&`)

## Success Metrics

- Zero functional regression in Plex auth, server discovery, library browsing, and media sync
- `xml.zig` eliminated (171 lines of custom parser removed)
- Entity decoding bugs fixed automatically by switching to JSON

## Dependencies & Risks

**Risk: Older PMS versions might not support JSON Accept header.** Mitigation: Plex has supported JSON responses for many years. This is unlikely to be an issue for actively maintained servers.

**Risk: PIN endpoint JSON behavior.** Some forum reports suggest PIN endpoints may behave differently with JSON in JWT auth flows. Mitigation: Test PIN auth flow end-to-end after migration. The classic PIN flow (non-JWT) works reliably with JSON.

**Risk: Field type mismatches in JSON responses.** A field that's always present in XML might be absent or differently typed in JSON. Mitigation: Dynamic `std.json.Value` extraction with `orelse` patterns handles this gracefully, matching current XML resilience.

## MVP

### Stage 1: Change Accept header + add JSON helpers

**Goal**: Change the wire format to JSON, add helper functions for JSON value extraction.
**Files**: `src/net/plex/types.zig`

```zig
// types.zig - change Accept header
buf[i] = .{ .name = "Accept", .value = "application/json" };
```

Add JSON extraction helpers (similar to TMDB's `dupeJsonString`/`dupeOptionalJsonString` pattern) either in a new `json_helpers.zig` or directly in `client.zig`/`auth.zig`.

### Stage 2: Migrate auth.zig (PIN endpoints)

**Goal**: Replace XML parsing in `requestPin()` and `pollPin()` with JSON.
**Files**: `src/net/plex/auth.zig`

PIN responses are the simplest -- bare JSON objects with `id`, `code`, `authToken` at the top level.

```zig
// auth.zig - requestPin() JSON parsing
const parsed = std.json.parseFromSlice(std.json.Value, self.allocator, response.body, .{}) catch
    return error.InvalidResponse;
defer parsed.deinit();

const root = parsed.value.object;
const id = root.get("id") orelse return error.InvalidResponse;
const code_val = root.get("code") orelse return error.InvalidResponse;

self.pin_id = switch (id) {
    .integer => |i| i,
    else => return error.InvalidResponse,
};
const code = switch (code_val) {
    .string => |s| s,
    else => return error.InvalidResponse,
};

if (self.pin_code) |old| self.allocator.free(old);
self.pin_code = try self.allocator.dupe(u8, code);
self.state = .awaiting_pin;
return self.pin_code.?;
```

### Stage 3: Migrate discoverServers (resources endpoint)

**Goal**: Replace XML parsing in `discoverServers()` with JSON.
**Files**: `src/net/plex/client.zig`

The resources endpoint returns a bare JSON array. The stateful XML iteration (`in_server_device`, `current_server`, `current_connections`) can be replaced with a simple array loop with nested `connections` access.

```zig
// client.zig - discoverServers() JSON parsing
const parsed = std.json.parseFromSlice(std.json.Value, self.allocator, response.body, .{}) catch
    return error.RequestFailed;
defer parsed.deinit();

const devices = switch (parsed.value) {
    .array => |a| a,
    else => return error.RequestFailed,
};

var servers: std.ArrayList(plex_types.PlexServer) = .{};

for (devices.items) |device| {
    const obj = switch (device) {
        .object => |o| o,
        else => continue,
    };

    const provides = switch (obj.get("provides") orelse continue) {
        .string => |s| s,
        else => continue,
    };
    if (std.mem.indexOf(u8, provides, "server") == null) continue;

    const name = switch (obj.get("name") orelse continue) {
        .string => |s| s,
        else => continue,
    };
    const client_id = switch (obj.get("clientIdentifier") orelse continue) {
        .string => |s| s,
        else => continue,
    };

    // Parse nested connections array
    var conns: std.ArrayList(plex_types.PlexServer.Connection) = .{};
    if (obj.get("connections")) |conns_val| {
        if (conns_val == .array) {
            for (conns_val.array.items) |conn| {
                const c = switch (conn) {
                    .object => |o| o,
                    else => continue,
                };
                const uri = switch (c.get("uri") orelse continue) {
                    .string => |s| s,
                    else => continue,
                };
                try conns.append(self.allocator, .{
                    .uri = try self.allocator.dupe(u8, uri),
                    .local = if (c.get("local")) |v| v == .true else false,
                    .relay = if (c.get("relay")) |v| v == .true else false,
                    .protocol = try self.allocator.dupe(u8,
                        if (c.get("protocol")) |v| switch (v) { .string => |s| s, else => "https" } else "https"),
                });
            }
        }
    }

    try servers.append(self.allocator, .{
        .name = try self.allocator.dupe(u8, name),
        .machine_identifier = try self.allocator.dupe(u8, client_id),
        .access_token = if (obj.get("accessToken")) |v| switch (v) {
            .string => |s| try self.allocator.dupe(u8, s),
            else => null,
        } else null,
        .connections = conns.toOwnedSlice(self.allocator) catch &.{},
    });
}

return servers.toOwnedSlice(self.allocator);
```

### Stage 4: Migrate getLibraries + fetchMediaItems (PMS endpoints)

**Goal**: Replace XML parsing in `getLibraries()` and `fetchMediaItems()` with JSON.
**Files**: `src/net/plex/client.zig`

Both use the `MediaContainer` wrapper. `getLibraries` reads `Directory` array, `fetchMediaItems` reads `Metadata` array.

```zig
// client.zig - getLibraries() JSON parsing
const parsed = std.json.parseFromSlice(std.json.Value, self.allocator, response.body, .{}) catch
    return error.RequestFailed;
defer parsed.deinit();

const container = switch (parsed.value) {
    .object => |o| o,
    else => return error.RequestFailed,
};
const mc = switch (container.get("MediaContainer") orelse return &.{}) {
    .object => |o| o,
    else => return &.{},
};
const dirs = switch (mc.get("Directory") orelse return &.{}) {
    .array => |a| a,
    else => return &.{},
};

var libraries: std.ArrayList(plex_types.PlexLibrary) = .{};
for (dirs.items) |item| {
    const obj = switch (item) { .object => |o| o, else => continue };
    try libraries.append(self.allocator, .{
        .key = try self.allocator.dupe(u8, switch (obj.get("key") orelse continue) { .string => |s| s, else => continue }),
        .title = try self.allocator.dupe(u8, switch (obj.get("title") orelse continue) { .string => |s| s, else => continue }),
        .library_type = try self.allocator.dupe(u8, switch (obj.get("type") orelse continue) { .string => |s| s, else => continue }),
    });
}
return libraries.toOwnedSlice(self.allocator);
```

```zig
// client.zig - fetchMediaItems() JSON parsing
const parsed = std.json.parseFromSlice(std.json.Value, self.allocator, response.body, .{}) catch
    return error.RequestFailed;
defer parsed.deinit();

const container = switch (parsed.value) {
    .object => |o| o,
    else => return error.RequestFailed,
};
const mc = switch (container.get("MediaContainer") orelse return &.{}) {
    .object => |o| o,
    else => return &.{},
};
const metadata = switch (mc.get("Metadata") orelse return &.{}) {
    .array => |a| a,
    else => return &.{},
};

var items: std.ArrayList(plex_types.PlexMediaItem) = .{};
for (metadata.items) |entry| {
    const obj = switch (entry) { .object => |o| o, else => continue };
    try items.append(self.allocator, .{
        .rating_key = try self.allocator.dupe(u8, switch (obj.get("ratingKey") orelse continue) { .string => |s| s, else => continue }),
        .title = try self.allocator.dupe(u8, switch (obj.get("title") orelse continue) { .string => |s| s, else => continue }),
        .media_type = try self.allocator.dupe(u8, switch (obj.get("type") orelse continue) { .string => |s| s, else => "unknown" }),
        .summary = dupeOptionalJsonString(self.allocator, obj.get("summary")),
        .year = if (obj.get("year")) |v| switch (v) { .integer => |i| @intCast(i), else => null } else null,
        .rating = if (obj.get("rating")) |v| switch (v) { .float => |f| f, .integer => |i| @floatFromInt(i), else => null } else null,
        .duration_ms = if (obj.get("duration")) |v| switch (v) { .integer => |i| i, else => null } else null,
        .thumb = dupeOptionalJsonString(self.allocator, obj.get("thumb")),
        .art = dupeOptionalJsonString(self.allocator, obj.get("art")),
        .parent_rating_key = dupeOptionalJsonString(self.allocator, obj.get("parentRatingKey")),
        .grandparent_rating_key = dupeOptionalJsonString(self.allocator, obj.get("grandparentRatingKey")),
        .grandparent_title = dupeOptionalJsonString(self.allocator, obj.get("grandparentTitle")),
        .parent_index = if (obj.get("parentIndex")) |v| switch (v) { .integer => |i| @intCast(i), else => null } else null,
        .index = if (obj.get("index")) |v| switch (v) { .integer => |i| @intCast(i), else => null } else null,
        .view_offset = if (obj.get("viewOffset")) |v| switch (v) { .integer => |i| i, else => null } else null,
        .part_key = dupeOptionalJsonString(self.allocator, obj.get("key")),
    });
}
return items.toOwnedSlice(self.allocator);
```

### Stage 5: Cleanup

**Goal**: Remove dead XML code and imports.
**Files**: `src/net/plex/xml.zig`, `src/net/plex/client.zig`, `src/net/plex/auth.zig`, `src/lib.zig`

- Delete `src/net/plex/xml.zig`
- Remove `const xml = @import("xml.zig");` from `client.zig` and `auth.zig`
- Remove `pub const plex_xml = @import("net/plex/xml.zig");` from `lib.zig` line 17

### Test Plan

Each stage should include tests with inline JSON literals matching real Plex response structures:

```zig
test "parse PIN response JSON" {
    const json =
        \\{"id":308667304,"code":"7RQZ","product":"Reel","trusted":false,"authToken":null}
    ;
    // Parse and verify id=308667304, code="7RQZ", authToken=null
}

test "parse resources JSON (bare array)" {
    const json =
        \\[{"name":"My Server","clientIdentifier":"abc123","provides":"server","accessToken":"tok","connections":[{"uri":"https://192.168.1.1:32400","local":true,"relay":false,"protocol":"https"}]}]
    ;
    // Parse and verify 1 server with 1 connection
}

test "parse MediaContainer with Metadata" {
    const json =
        \\{"MediaContainer":{"size":1,"Metadata":[{"ratingKey":"123","title":"Test Movie","type":"movie","year":2024,"rating":7.5,"duration":7200000,"summary":"A test film","thumb":"/thumb/123","art":"/art/123","key":"/library/metadata/123"}]}}
    ;
    // Parse and verify all fields
}

test "parse empty MediaContainer" {
    const json =
        \\{"MediaContainer":{"size":0}}
    ;
    // Parse and verify empty items array (no "Metadata" key)
}
```

## Sources

- [Plex API JSON format](https://developer.plex.tv/pms/) -- JSON field names and structures
- [LukeHagar/plex-api-spec](https://github.com/LukeHagar/plex-api-spec) -- Community OpenAPI spec for Plex
- Existing TMDB client pattern: `src/net/tmdb/client.zig` -- reference for `std.json.Value` extraction
- Zig `std.json` docs: `std/json/static.zig` -- `parseFromSlice` API
- Best practices doc: `docs/research/best-practices-research.md` -- recommends JSON over XML for Zig
