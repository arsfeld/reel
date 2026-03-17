# Connection Strategy

**Rule:** Always use relay first, test local connections in parallel in a background thread, switch to local URL if and only if one becomes available.

This avoids blocking the UI on slow/unreachable local connection tests. Relay connections are always reachable (Plex guarantees this), so use them immediately for the fastest startup.
