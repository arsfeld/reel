const std = @import("std");
const http = @import("http.zig");
const types = @import("../core/types.zig");

fn milliTimestamp() i64 {
    var ts: std.c.timespec = undefined;
    _ = std.c.clock_gettime(.REALTIME, &ts);
    return @as(i64, @intCast(ts.sec)) * 1000 + @divTrunc(@as(i64, @intCast(ts.nsec)), 1_000_000);
}

pub const TestResult = struct {
    uri: []const u8,
    score: i32,
    latency_ms: i32,
    is_local: bool,
    is_relay: bool,
};

pub const ConnectionSelector = struct {
    allocator: std.mem.Allocator,
    http_client: *http.HttpClient,
    plex_headers: []const http.Header,

    pub fn init(
        allocator: std.mem.Allocator,
        http_client: *http.HttpClient,
        plex_headers: []const http.Header,
    ) ConnectionSelector {
        return .{
            .allocator = allocator,
            .http_client = http_client,
            .plex_headers = plex_headers,
        };
    }

    /// Test a single connection URI. Returns result if reachable, null otherwise.
    pub fn testConnection(self: *ConnectionSelector, uri: []const u8, is_local: bool, is_relay: bool) ?TestResult {
        const url = std.fmt.allocPrint(self.allocator, "{s}/identity", .{uri}) catch return null;
        defer self.allocator.free(url);

        const start = milliTimestamp();

        var response = self.http_client.get(url, self.plex_headers) catch {
            return null;
        };
        defer response.deinit();

        if (response.status != .ok) return null;

        const latency: i32 = @intCast(@min(milliTimestamp() - start, std.math.maxInt(i32)));

        return TestResult{
            .uri = uri,
            .score = 0,
            .latency_ms = latency,
            .is_local = is_local,
            .is_relay = is_relay,
        };
    }

    /// Relay-first connection strategy:
    /// Returns the best relay/remote URI immediately (no network tests).
    /// Use testLocalConnections() in a background thread to upgrade later.
    pub fn selectImmediate(connections: []const types.ServerConnection) ?[]const u8 {
        // 1. Prefer relay (always reachable via Plex cloud)
        for (connections) |conn| {
            if (conn.is_relay) return conn.uri;
        }
        // 2. Prefer remote (non-local, non-relay)
        for (connections) |conn| {
            if (!conn.is_local and !conn.is_relay) return conn.uri;
        }
        // 3. Fallback to first available
        if (connections.len > 0) return connections[0].uri;
        return null;
    }

    /// Test local connections (call from background thread).
    /// Returns the first working local URI, or null if none work.
    pub fn findWorkingLocal(self: *ConnectionSelector, connections: []const types.ServerConnection) ?TestResult {
        for (connections) |conn| {
            if (!conn.is_local) continue;
            if (self.testConnection(conn.uri, true, false)) |result| {
                return result;
            }
        }
        return null;
    }
};

test "ConnectionSelector init" {
    var hc = http.HttpClient.init(std.testing.allocator);
    defer hc.deinit();

    var selector = ConnectionSelector.init(std.testing.allocator, &hc, &.{});
    _ = &selector;
}
