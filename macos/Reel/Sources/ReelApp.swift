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
    }

    deinit {
        if let dl = downloader { reel_download_destroy(dl) }
        if let lib = library { reel_library_destroy(lib) }
        if let d = db { reel_db_close(d) }
    }
}
