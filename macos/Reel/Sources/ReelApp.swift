import SwiftUI
import AppKit
import ReelCore

@main
struct ReelApp: App {
    @State private var appState = AppState()

    init() {
        NSApplication.shared.setActivationPolicy(.regular)
        NSApplication.shared.activate()
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(appState)
                .frame(minWidth: 800, minHeight: 500)
                .preferredColorScheme(.dark)
                .onAppear {
                    appState.initialize()
                }
        }
        .defaultSize(width: 1280, height: 720)
    }
}

@Observable
final class AppState {
    var db: OpaquePointer?
    var library: OpaquePointer?
    var downloader: OpaquePointer?
    var isInitialized = false
    var isSyncing = false
    var syncStatus: String = ""
    var lastSyncTime: Date?
    private var connectionTimer: Timer?

    func initialize() {
        guard !isInitialized else { return }

        let appSupport = FileManager.default.urls(
            for: .applicationSupportDirectory,
            in: .userDomainMask
        ).first!.appendingPathComponent("Reel")

        try? FileManager.default.createDirectory(
            at: appSupport,
            withIntermediateDirectories: true
        )

        let dbPath = appSupport.appendingPathComponent("reel.db").path
        db = reel_db_open(dbPath)
        guard db != nil else {
            print("Failed to open database at \(dbPath)")
            return
        }

        library = reel_library_create(db)
        downloader = reel_download_create(db)
        isInitialized = true

        // Background: sync immediately with stored URI, resolve best connection in parallel
        if let lib = library {
            let libPtr = lib

            // Use a proper thread with large stack for Zig C calls (Swift Task has limited stack)
            isSyncing = true
            syncStatus = "Syncing library..."

            // 1. Refresh connection list from Plex API, then sync immediately
            let syncThread = Thread { [weak self] in
                // Quick API call to get latest connection URIs
                reel_server_refresh_connections(libPtr)

                // Sync now using whatever connection_uri is already stored
                let count = reel_plex_sync_all(libPtr)
                if count > 0 {
                    print("Synced \(count) items from Plex servers")
                }
                DispatchQueue.main.async {
                    self?.isSyncing = false
                    self?.syncStatus = ""
                    self?.lastSyncTime = Date()
                }
            }
            syncThread.stackSize = 8 * 1024 * 1024  // 8MB stack
            syncThread.start()

            // 2. Resolve best connection in parallel (slow — tests each URI for latency)
            //    Updates stored connection_uri for future syncs
            let resolveThread = Thread {
                let servers = ReelBridge.listServers(library: libPtr)
                for server in servers {
                    if let uri = ReelBridge.resolveConnection(library: libPtr, serverId: server.id) {
                        print("Resolved best connection for '\(server.name)': \(uri)")
                    }
                }
            }
            resolveThread.stackSize = 8 * 1024 * 1024
            resolveThread.start()
        }

        // Periodic re-resolution every 60s
        startConnectionTimer()
    }

    private func startConnectionTimer() {
        guard let lib = library else { return }
        let libPtr = lib
        connectionTimer = Timer.scheduledTimer(withTimeInterval: 60, repeats: true) { _ in
            Task.detached {
                // Refresh connections from Plex API
                reel_server_refresh_connections(libPtr)

                // Resolve best for each server
                let servers = ReelBridge.listServers(library: libPtr)
                for server in servers {
                    if let uri = ReelBridge.resolveConnection(library: libPtr, serverId: server.id) {
                        print("Re-resolved connection for '\(server.name)': \(uri)")
                    }
                }
            }
        }
    }

    deinit {
        connectionTimer?.invalidate()
        if let dl = downloader { reel_download_destroy(dl) }
        if let lib = library { reel_library_destroy(lib) }
        if let d = db { reel_db_close(d) }
    }
}
