import SwiftUI
import Combine

// Placeholder types until Swift bridge is working
struct Backend: Identifiable {
    let id: String
    let name: String
    let type: String
}

struct Library: Identifiable {
    let id: String
    let name: String
    let type: String
    let backendId: String
}

struct MediaItem: Identifiable {
    let id: String
    let title: String
    let year: String
    let type: String
    let libraryId: String
}

@MainActor
class AppModel: ObservableObject {
    @Published var isInitialized = false
    @Published var backends: [Backend] = []
    @Published var libraries: [Library] = []
    @Published var mediaItems: [MediaItem] = []
    @Published var isLoading = false
    @Published var errorMessage: String?
    
    // Swift bridge integration - will be enabled when bridge files are generated
    #if canImport(Generated)
    private var core: MacOSCore?
    private var eventSubscriber: EventSub?
    #endif
    
    init() {
        Task {
            await initialize()
        }
    }
    
    private func initialize() async {
        print("Initializing Reel app model...")
        
        #if canImport(Generated)
        // Try to initialize Rust core if Swift bridge is available
        do {
            core = MacOSCore()
            if let core = core {
                core.sb_initialize()
                isInitialized = core.sb_is_initialized()
                
                if isInitialized {
                    print("Rust core initialized successfully")
                    await loadBackends()
                    await startEventListener()
                    return
                }
            }
        } catch {
            print("Failed to initialize Rust core: \(error)")
            errorMessage = "Failed to initialize: \(error)"
        }
        #else
        print("Swift bridge not available, using mock data")
        #endif
        
        // Fallback to mock data if Rust core is not available
        await loadMockData()
        isInitialized = true
    }
    
    private func loadMockData() async {
        // Mock data for UI development
        backends = [
            Backend(id: "plex-1", name: "My Plex Server", type: "plex"),
            Backend(id: "jellyfin-1", name: "Home Jellyfin", type: "jellyfin")
        ]
        
        libraries = [
            Library(id: "lib-1", name: "Movies", type: "movie", backendId: "plex-1"),
            Library(id: "lib-2", name: "TV Shows", type: "show", backendId: "plex-1"),
            Library(id: "lib-3", name: "Movies", type: "movie", backendId: "jellyfin-1")
        ]
        
        mediaItems = [
            MediaItem(id: "item-1", title: "Example Movie", year: "2024", type: "movie", libraryId: "lib-1"),
            MediaItem(id: "item-2", title: "Another Movie", year: "2023", type: "movie", libraryId: "lib-1"),
            MediaItem(id: "item-3", title: "TV Show", year: "2022", type: "show", libraryId: "lib-2")
        ]
    }
    
    private func loadBackends() async {
        #if canImport(Generated)
        if let core = core {
            let backendBridges = core.list_backends()
            backends = backendBridges.map { bridge in
                Backend(
                    id: String(bridge.id),
                    name: String(bridge.name),
                    type: String(bridge.backend_type)
                )
            }
            
            // Also load cached libraries for each backend
            for backend in backends {
                let libraryBridges = core.get_cached_libraries(backendId: backend.id)
                let backendLibraries = libraryBridges.map { bridge in
                    Library(
                        id: String(bridge.id),
                        name: String(bridge.name),
                        type: String(bridge.library_type),
                        backendId: backend.id
                    )
                }
                libraries.append(contentsOf: backendLibraries)
            }
        }
        #endif
    }
    
    private func startEventListener() async {
        #if canImport(Generated)
        guard let core = core else { return }
        
        let eventKinds = ["MediaCreated", "MediaUpdated", "LibraryCreated", "SourceCreated"]
        eventSubscriber = core.subscribe(eventKinds: eventKinds)
        
        Task {
            while let eventSub = eventSubscriber {
                if let event = eventSub.next_event_blocking(timeoutMs: 1000) {
                    await handleEvent(event)
                } else {
                    // Timeout occurred, check if we should continue
                    if !isInitialized { break }
                }
            }
        }
        #endif
    }
    
    private func handleEvent(_ event: Any) async {
        // TODO: Handle events from Rust
        print("Received event: \(event)")
    }
    
    func refreshLibraries() {
        Task {
            isLoading = true
            await loadBackends()
            // TODO: Trigger sync in Rust core
            isLoading = false
        }
    }
    
    func getBackend(id: String) -> Backend? {
        backends.first { $0.id == id }
    }
    
    func getLibrary(id: String) -> Library? {
        libraries.first { $0.id == id }
    }
    
    func getMediaItems(for backendId: String) -> [MediaItem] {
        let libraryIds = libraries
            .filter { $0.backendId == backendId }
            .map { $0.id }
        
        return mediaItems.filter { libraryIds.contains($0.libraryId) }
    }
    
    func showAbout() {
        NSApp.orderFrontStandardAboutPanel(options: [:])
    }
}