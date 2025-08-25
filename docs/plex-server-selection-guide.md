# Plex Server Selection and Connection Management Guide

## Overview

This document provides a comprehensive explanation of how Reel selects the best Plex server connection and manages these selections through caching and persistence. The system implements an intelligent connection selection algorithm with offline-first capabilities and automatic failover.

## Key Components

### 1. AuthManager (`src/services/auth_manager.rs`)
The central service that manages authentication providers and server discovery.

### 2. PlexAuth (`src/backends/plex/auth.rs`)
Handles Plex authentication and server discovery API calls.

### 3. Config (`src/config.rs`)
Persists authentication providers and cached server information.

### 4. Models (`src/models/auth_provider.rs`)
Defines data structures for authentication providers and sources.

## Server Discovery Process

### Step 1: Authentication
When a user adds a Plex account via `AuthManager::add_plex_account()`:

1. **Token Storage**: The Plex authentication token is stored securely in the system keyring
2. **User Info Retrieval**: Fetches user details from Plex API
3. **Provider Creation**: Creates an `AuthProvider::PlexAccount` instance
4. **Persistence**: Saves provider to config and keyring

### Step 2: Server Discovery
The `AuthManager::discover_plex_sources()` method:

1. **Cache Check**: First checks for cached sources (5-minute freshness window)
2. **Network Fetch**: If cache is stale, calls `PlexAuth::discover_servers()`
3. **Server Filtering**: Only includes servers that provide "server" capability
4. **Connection Processing**: For each server, processes all available connections

## Connection Selection Algorithm

The system implements a sophisticated connection selection algorithm in `auth_manager.rs` lines 270-300:

### Connection Priority Levels

```rust
// Sort connections by preference (lines 278-289)
sorted_connections.sort_by_key(|c| {
    if c.local && !c.relay {
        0  // Best: local non-relay (direct LAN connection)
    } else if c.local {
        1  // Good: local (might be relay)
    } else if !c.relay {
        2  // OK: remote direct connection
    } else {
        3  // Last resort: relay through Plex
    }
});
```

### Priority Explanation

1. **Priority 0 - Local Direct** (`local: true, relay: false`)
   - Direct LAN connection to server
   - Fastest performance, no external dependencies
   - Example: `http://192.168.1.100:32400`

2. **Priority 1 - Local (Possibly Relay)** (`local: true, relay: varies`)
   - Marked as local but might use relay
   - Good performance, preferred over remote

3. **Priority 2 - Remote Direct** (`local: false, relay: false`)
   - Direct connection over internet
   - Requires port forwarding on server
   - Example: `https://my-server.example.com:32400`

4. **Priority 3 - Relay** (`local: false, relay: true`)
   - Connection through Plex relay servers
   - Slowest but works without port forwarding
   - Example: `https://1-2-3-4.relay.plex.tv:32400`

### Selection Process

1. **Connection Discovery**: Plex API returns multiple connections per server
2. **Logging**: All available connections are logged for debugging (lines 272-275)
3. **Sorting**: Connections sorted by priority algorithm
4. **Selection**: First connection in sorted list is selected as primary
5. **Storage**: Selected URL stored in `source.connection_info.primary_url`

## Caching and Persistence

### Three-Layer Storage Architecture

#### 1. In-Memory Cache (AuthManager)
- `providers: Arc<RwLock<HashMap<String, AuthProvider>>>`
- Immediate access for active session
- Lost on application restart

#### 2. Config File Cache (`~/.config/reel/config.toml`)
- Persisted TOML configuration
- Stores:
  - `auth_providers`: Provider metadata (without sensitive data)
  - `cached_sources`: Discovered servers and their URLs
  - `sources_last_fetched`: Cache timestamps
  - `library_visibility`: User preferences

#### 3. Keyring Storage (System Secure Storage)
- Sensitive credentials (tokens, passwords)
- Uses `keyring` crate with service name "dev.arsfeld.Reel"
- Key format: `{provider_id}_{field}` (e.g., "plex_abc123_token")

### Cache Lifecycle

#### Discovery and Caching
```rust
// From auth_manager.rs lines 225-328
pub async fn discover_plex_sources(&self, provider_id: &str) -> Result<Vec<Source>> {
    // 1. Check cache freshness (5-minute window)
    let cached = config.get_cached_sources(provider_id);
    let is_stale = config.is_sources_cache_stale(provider_id, 300);
    
    // 2. Return cached if fresh
    if let Some(ref sources) = cached {
        if !is_stale {
            return Ok(sources.clone());
        }
    }
    
    // 3. Fetch fresh data from network
    match PlexAuth::discover_servers(token).await {
        Ok(servers) => {
            // 4. Process and cache
            config.set_cached_sources(provider_id.to_string(), sources.clone());
        }
        Err(e) => {
            // 5. Fallback to cached data if network fails
            if let Some(sources) = cached {
                return Ok(sources);
            }
        }
    }
}
```

### URL Updates

The system can update a source's URL when a better connection is found:

```rust
// auth_manager.rs lines 362-380
pub async fn update_source_url(&self, source_id: &str, new_url: &str) -> Result<()> {
    // Updates the URL in cached_sources
    // Persists to config file
}
```

This allows dynamic optimization of connections based on network conditions.

## Offline-First Design

### Graceful Degradation
1. **Always Try Cache First**: Returns cached data immediately if available
2. **Background Refresh**: Can refresh sources without blocking UI
3. **Network Failure Handling**: Falls back to cached data on network errors
4. **Stale Data Tolerance**: Uses stale cache if network unavailable

### Background Refresh
```rust
// auth_manager.rs lines 383-393
pub async fn refresh_sources_background(&self, provider_id: &str) {
    tokio::spawn(async move {
        // Refreshes sources without blocking
        self_clone.discover_plex_sources(&provider_id).await;
    });
}
```

## Migration Support

The system includes legacy backend migration (lines 471-670):

1. **Detection**: Identifies legacy Plex backends
2. **Token Recovery**: Retrieves tokens from old keyring locations
3. **Provider Creation**: Creates new AuthProvider instances
4. **Cleanup**: Removes migrated backends from legacy list

## Connection Testing Strategy

While the current implementation selects connections based on static priorities, the architecture supports future enhancements:

1. **Parallel Testing**: Could test multiple connections simultaneously
2. **Latency Measurement**: Select based on actual response times
3. **Automatic Failover**: Switch to backup connections on failure
4. **Geographic Optimization**: Prefer geographically closer servers

## Data Flow Diagram

```
User adds Plex Account
        ↓
    AuthManager
        ↓
    PlexAuth API
        ↓
  Server Discovery
        ↓
  Connection List
        ↓
  Priority Sorting
        ↓
  URL Selection
        ↓
    Caching
    ├── Memory
    ├── Config File
    └── Keyring
        ↓
  Source Created
        ↓
  Ready for Use
```

## Best Practices

### For Optimal Connection Selection

1. **Port Forwarding**: Configure port forwarding on Plex server for direct remote access
2. **Static IP/Domain**: Use static IPs or dynamic DNS for reliable remote connections
3. **Local Network**: Ensure devices are on same network for LAN discovery
4. **Firewall Rules**: Allow Plex port (32400) through firewalls

### For Developers

1. **Cache Invalidation**: Respect cache TTL (currently 5 minutes)
2. **Error Handling**: Always provide fallback to cached data
3. **Logging**: Log connection attempts for debugging
4. **Security**: Never log full tokens, only first few characters

## Future Improvements

Potential enhancements to the connection selection system:

1. **Dynamic Testing**: Test connections in real-time before selection
2. **Performance Metrics**: Track and store connection performance history
3. **User Preferences**: Allow manual connection preference settings
4. **Adaptive TTL**: Adjust cache TTL based on network stability
5. **Connection Pooling**: Maintain multiple active connections for failover

## Troubleshooting

### Common Issues

1. **Wrong Server Selected**
   - Clear cache: Remove sources from config
   - Check network: Ensure LAN discovery works
   - Verify ports: Check port forwarding settings

2. **Slow Connections**
   - Check if using relay (Priority 3)
   - Verify local network connectivity
   - Review firewall settings

3. **Authentication Failures**
   - Check keyring permissions
   - Verify token validity
   - Review migration status for legacy backends

## Summary

The Plex server selection system in Reel implements a sophisticated, offline-first approach to connection management. It prioritizes local direct connections for best performance while providing automatic fallbacks for reliability. The multi-layer caching system ensures fast startup times and resilience to network failures, while the secure credential storage protects user tokens. The architecture is designed for future enhancements while maintaining backward compatibility with legacy configurations.