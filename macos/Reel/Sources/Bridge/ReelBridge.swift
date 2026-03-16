import Foundation
import ReelCore

/// Swift-friendly wrappers around the C ABI functions.
/// All functions that return C strings take ownership and free them via reel_free.
enum ReelBridge {

    // MARK: - Library Queries

    static func getItems(
        library lib: OpaquePointer,
        type mediaType: MediaType,
        sortBy: SortField = .title,
        sortOrder: SortOrder = .asc,
        limit: Int32 = 50,
        offset: Int32 = 0
    ) -> [MediaItem] {
        let maxItems = limit
        var buffer = [ReelMediaItem](repeating: ReelMediaItem(), count: Int(maxItems))
        let count = reel_library_get_items(
            lib,
            ReelMediaType(rawValue: UInt32(mediaType.rawValue)),
            ReelSortField(rawValue: UInt32(sortBy.rawValue)),
            ReelSortOrder(rawValue: UInt32(sortOrder.rawValue)),
            limit, offset, &buffer, maxItems
        )
        guard count > 0 else { return [] }
        return (0..<Int(count)).map { MediaItem(from: buffer[$0]) }
    }

    static func getRecentlyAdded(library lib: OpaquePointer, limit: Int32 = 20) -> [MediaItem] {
        var buffer = [ReelMediaItem](repeating: ReelMediaItem(), count: Int(limit))
        let count = reel_library_get_recently_added(lib, limit, &buffer, limit)
        guard count > 0 else { return [] }
        return (0..<Int(count)).map { MediaItem(from: buffer[$0]) }
    }

    static func getContinueWatching(library lib: OpaquePointer, limit: Int32 = 20) -> [MediaItem] {
        var buffer = [ReelMediaItem](repeating: ReelMediaItem(), count: Int(limit))
        let count = reel_library_get_continue_watching(lib, limit, &buffer, limit)
        guard count > 0 else { return [] }
        return (0..<Int(count)).map { MediaItem(from: buffer[$0]) }
    }

    static func search(library lib: OpaquePointer, query: String) -> [MediaItem] {
        let maxItems: Int32 = 50
        var buffer = [ReelMediaItem](repeating: ReelMediaItem(), count: Int(maxItems))
        let count = reel_library_search(lib, query, &buffer, maxItems)
        guard count > 0 else { return [] }
        return (0..<Int(count)).map { MediaItem(from: buffer[$0]) }
    }

    static func getItem(library lib: OpaquePointer, id: Int64) -> MediaItem? {
        var item = ReelMediaItem()
        let err = reel_library_get_item(lib, id, &item)
        guard err.rawValue == 0 else { return nil }
        return MediaItem(from: item)
    }

    static func getItemsByParent(library lib: OpaquePointer, parentId: Int64) -> [MediaItem] {
        let maxItems: Int32 = 200
        var buffer = [ReelMediaItem](repeating: ReelMediaItem(), count: Int(maxItems))
        let count = reel_library_get_items_by_parent(lib, parentId, &buffer, maxItems)
        guard count > 0 else { return [] }
        return (0..<Int(count)).map { MediaItem(from: buffer[$0]) }
    }

    static func getItemCount(library lib: OpaquePointer, type mediaType: MediaType) -> Int32 {
        reel_library_get_item_count(lib, ReelMediaType(rawValue: UInt32(mediaType.rawValue)))
    }

    // MARK: - Genres

    static func getGenres(library lib: OpaquePointer) -> [String] {
        var count: Int32 = 0
        reel_library_get_genres(lib, nil, &count)
        guard count > 0 else { return [] }

        var buffer = [ReelGenreC](repeating: ReelGenreC(), count: Int(count))
        reel_library_get_genres(lib, &buffer, &count)
        return (0..<Int(count)).map { String(cString: buffer[$0].name) }
    }

    static func getItemsByGenre(library lib: OpaquePointer, genre: String, limit: Int32 = 20) -> [MediaItem] {
        var buffer = [ReelMediaItem](repeating: ReelMediaItem(), count: Int(limit))
        let count = reel_library_get_items_by_genre(lib, genre, limit, &buffer, limit)
        guard count > 0 else { return [] }
        return (0..<Int(count)).map { MediaItem(from: buffer[$0]) }
    }

    // MARK: - Favorites

    struct Favorite: Identifiable {
        let id: Int64
        let itemType: String
        let itemId: String
        let displayName: String
        let sortOrder: Int32
    }

    static func listFavorites(library lib: OpaquePointer) -> [Favorite] {
        var count: Int32 = 0
        reel_library_list_favorites(lib, nil, &count)
        guard count > 0 else { return [] }

        var buffer = [ReelFavoriteC](repeating: ReelFavoriteC(), count: Int(count))
        reel_library_list_favorites(lib, &buffer, &count)
        return (0..<Int(count)).map { i in
            Favorite(
                id: buffer[i].id,
                itemType: String(cString: buffer[i].item_type),
                itemId: String(cString: buffer[i].item_id),
                displayName: String(cString: buffer[i].display_name),
                sortOrder: buffer[i].sort_order
            )
        }
    }

    // MARK: - Scan Paths

    struct ScanPath: Identifiable {
        let id: Int64
        let path: String
    }

    static func listScanPaths(library lib: OpaquePointer) -> [ScanPath] {
        var count: Int32 = 0
        reel_library_list_scan_paths(lib, nil, &count)
        guard count > 0 else { return [] }

        var buffer = [ReelScanPathC](repeating: ReelScanPathC(), count: Int(count))
        reel_library_list_scan_paths(lib, &buffer, &count)
        return (0..<Int(count)).map { i in
            ScanPath(id: buffer[i].id, path: String(cString: buffer[i].path))
        }
    }

    // MARK: - Servers

    struct Server: Identifiable {
        let id: String
        let name: String
        let connectionUri: String
    }

    static func listServers(library lib: OpaquePointer) -> [Server] {
        var count: Int32 = 0
        reel_server_list(lib, nil, &count)
        guard count > 0 else { return [] }

        var buffer = [ReelServerC](repeating: ReelServerC(), count: Int(count))
        reel_server_list(lib, &buffer, &count)
        return (0..<Int(count)).map { i in
            Server(
                id: String(cString: buffer[i].id),
                name: String(cString: buffer[i].name),
                connectionUri: String(cString: buffer[i].connection_uri)
            )
        }
    }

    // MARK: - Connection Resolution

    /// Resolve the best connection URI for a server by testing all stored connections.
    /// Returns nil if no reachable connection found.
    static func resolveConnection(library lib: OpaquePointer, serverId: String) -> String? {
        guard let ptr = reel_server_resolve_connection(lib, serverId) else { return nil }
        let uri = String(cString: ptr)
        reel_free(UnsafeMutableRawPointer(mutating: ptr))
        return uri
    }

    /// Discover servers and store all connection URIs in the database.
    static func discoverServersWithLib(
        plex: OpaquePointer,
        library lib: OpaquePointer,
        buffer: inout [ReelPlexServerC],
        max: Int32
    ) -> Int32 {
        reel_plex_discover_servers_with_lib(plex, &buffer, max, lib)
    }

    /// Re-discover connections for all servers from Plex and store them in the DB.
    static func refreshConnections(library lib: OpaquePointer) -> Int32 {
        reel_server_refresh_connections(lib)
    }

    // MARK: - Watch Progress

    struct WatchProgress {
        let mediaItemId: Int64
        let positionMs: Int64
        let durationMs: Int64
        let watched: Bool
    }

    static func getWatchProgress(library lib: OpaquePointer, mediaItemId: Int64) -> WatchProgress? {
        var wp = ReelWatchProgressC()
        let err = reel_library_get_watch_progress(lib, mediaItemId, &wp)
        guard err.rawValue == 0 else { return nil }
        return WatchProgress(
            mediaItemId: wp.media_item_id,
            positionMs: wp.position_ms,
            durationMs: wp.duration_ms,
            watched: wp.watched != 0
        )
    }

    static func updateWatchProgress(
        library lib: OpaquePointer,
        mediaItemId: Int64,
        positionMs: Int64,
        durationMs: Int64,
        watched: Bool
    ) {
        reel_library_update_watch_progress(lib, mediaItemId, positionMs, durationMs, watched ? 1 : 0)
    }

    // MARK: - Downloads

    struct Download: Identifiable {
        let id: Int64
        let mediaItemId: Int64
        let status: String
        let downloadedBytes: Int64
        let totalBytes: Int64
        let localPath: String?
        let errorMessage: String?

        var progress: Double {
            guard totalBytes > 0 else { return 0 }
            return Double(downloadedBytes) / Double(totalBytes)
        }
    }

    static func listDownloads(downloader dl: OpaquePointer) -> [Download] {
        var count: Int32 = 0
        reel_download_list(dl, nil, &count)
        guard count > 0 else { return [] }

        var buffer = [ReelDownloadC](repeating: ReelDownloadC(), count: Int(count))
        reel_download_list(dl, &buffer, &count)
        return (0..<Int(count)).map { i in
            Download(
                id: buffer[i].id,
                mediaItemId: buffer[i].media_item_id,
                status: String(cString: buffer[i].status),
                downloadedBytes: buffer[i].downloaded_bytes,
                totalBytes: buffer[i].total_bytes,
                localPath: buffer[i].local_path.map { String(cString: $0) },
                errorMessage: buffer[i].error_message.map { String(cString: $0) }
            )
        }
    }

    // MARK: - Collections

    struct Collection: Identifiable {
        let id: Int64
        let name: String
        let collectionType: Int
        let description: String?
    }

    static func listCollections(library lib: OpaquePointer) -> [Collection] {
        var count: Int32 = 0
        reel_collection_list(lib, nil, &count)
        guard count > 0 else { return [] }

        var buffer = [ReelCollectionC](repeating: ReelCollectionC(), count: Int(count))
        reel_collection_list(lib, &buffer, &count)
        return (0..<Int(count)).map { i in
            Collection(
                id: buffer[i].id,
                name: String(cString: buffer[i].name),
                collectionType: Int(buffer[i].collection_type),
                description: buffer[i].description.map { String(cString: $0) }
            )
        }
    }

    static func getCollectionItems(library lib: OpaquePointer, collectionId: Int64) -> [MediaItem] {
        let maxItems: Int32 = 200
        var buffer = [ReelMediaItem](repeating: ReelMediaItem(), count: Int(maxItems))
        let count = reel_collection_get_items(lib, collectionId, &buffer, maxItems)
        guard count > 0 else { return [] }
        return (0..<Int(count)).map { MediaItem(from: buffer[$0]) }
    }
}

// MARK: - ReelMediaItem default initializer for buffer allocation

extension ReelMediaItem {
    init() {
        self.init(
            id: 0, media_type: 0, source: 0,
            title: UnsafePointer(bitPattern: 1)!, // non-null placeholder
            sort_title: nil, year: 0, summary: nil,
            rating: 0, duration_ms: 0,
            poster_path: nil, backdrop_path: nil,
            parent_id: 0, season_number: 0, episode_number: 0,
            file_path: nil
        )
    }
}

extension ReelGenreC {
    init() {
        self.init(id: 0, name: UnsafePointer(bitPattern: 1)!)
    }
}

extension ReelFavoriteC {
    init() {
        self.init(
            id: 0,
            item_type: UnsafePointer(bitPattern: 1)!,
            item_id: UnsafePointer(bitPattern: 1)!,
            display_name: UnsafePointer(bitPattern: 1)!,
            sort_order: 0
        )
    }
}

extension ReelScanPathC {
    init() {
        self.init(id: 0, path: UnsafePointer(bitPattern: 1)!)
    }
}

extension ReelServerC {
    init() {
        self.init(
            id: UnsafePointer(bitPattern: 1)!,
            name: UnsafePointer(bitPattern: 1)!,
            connection_uri: UnsafePointer(bitPattern: 1)!
        )
    }
}

extension ReelCollectionC {
    init() {
        self.init(
            id: 0,
            name: UnsafePointer(bitPattern: 1)!,
            collection_type: 0,
            description: nil
        )
    }
}

extension ReelDownloadC {
    init() {
        self.init(
            id: 0, media_item_id: 0,
            status: UnsafePointer(bitPattern: 1)!,
            downloaded_bytes: 0, total_bytes: 0,
            local_path: nil, error_message: nil
        )
    }
}

extension ReelWatchProgressC {
    init() {
        self.init(media_item_id: 0, position_ms: 0, duration_ms: 0, watched: 0)
    }
}
