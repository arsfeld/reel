const std = @import("std");
const http = @import("http.zig");
const types = @import("../core/types.zig");

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

    const SCORE_REACHABLE: i32 = 4;
    const SCORE_LOCAL: i32 = 2;
    const SCORE_SECURE: i32 = 1;

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

    pub fn testConnection(self: *ConnectionSelector, uri: []const u8, is_local: bool, is_relay: bool) ?TestResult {
        const url = std.fmt.allocPrint(self.allocator, "{s}/identity", .{uri}) catch return null;
        defer self.allocator.free(url);

        const start = std.time.milliTimestamp();

        var response = self.http_client.get(url, self.plex_headers) catch {
            return null; // Not reachable
        };
        defer response.deinit();

        if (response.status != .ok) return null;

        const latency: i32 = @intCast(@min(std.time.milliTimestamp() - start, std.math.maxInt(i32)));

        var score: i32 = SCORE_REACHABLE;
        if (is_local) score += SCORE_LOCAL;
        if (std.mem.startsWith(u8, uri, "https://")) score += SCORE_SECURE;

        return TestResult{
            .uri = uri,
            .score = score,
            .latency_ms = latency,
            .is_local = is_local,
            .is_relay = is_relay,
        };
    }

    pub fn selectBest(self: *ConnectionSelector, connections: []const types.ServerConnection) ?TestResult {
        var best: ?TestResult = null;

        for (connections) |conn| {
            std.log.info("connection_selector: testing uri='{s}' local={} relay={}", .{ conn.uri, conn.is_local, conn.is_relay });

            const result = self.testConnection(conn.uri, conn.is_local, conn.is_relay) orelse continue;

            std.log.info("connection_selector: uri='{s}' score={d} latency={d}ms", .{ conn.uri, result.score, result.latency_ms });

            if (best) |b| {
                if (result.score > b.score or (result.score == b.score and result.latency_ms < b.latency_ms)) {
                    best = result;
                }
            } else {
                best = result;
            }

            // Short-circuit: max possible score (reachable + local + secure) = 7
            if (result.score == SCORE_REACHABLE + SCORE_LOCAL + SCORE_SECURE) break;
        }

        return best;
    }
};

test "ConnectionSelector init" {
    var hc = http.HttpClient.init(std.testing.allocator);
    defer hc.deinit();

    var selector = ConnectionSelector.init(std.testing.allocator, &hc, &.{});
    _ = &selector;
}
