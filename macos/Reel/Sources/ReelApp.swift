import SwiftUI
import ReelCore

@main
struct ReelApp: App {
    @State private var appState = AppState()

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

        // Background: refresh connections from Plex, resolve best, then sync
        if let lib = library {
            let libPtr = lib
            Task.detached {
                // Refresh connections from Plex API (uses stored auth tokens)
                let refreshed = reel_server_refresh_connections(libPtr)
                print("Refreshed connections for \(refreshed) servers")

                // Resolve best connection for each server
                let servers = ReelBridge.listServers(library: libPtr)
                for server in servers {
                    if let uri = ReelBridge.resolveConnection(library: libPtr, serverId: server.id) {
                        print("Resolved connection for '\(server.name)': \(uri)")
                    }
                }

                // Sync libraries
                let count = reel_plex_sync_all(libPtr)
                if count > 0 {
                    print("Synced \(count) items from Plex servers")
                }
            }
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
