import SwiftUI

struct ContentView: View {
    @Environment(AppState.self) private var appState
    @State private var selectedItem: SidebarItem? = .home
    @State private var columnVisibility: NavigationSplitViewVisibility = .all
    @State private var playerModel = PlayerModel()

    var body: some View {
        ZStack {
            NavigationSplitView(columnVisibility: $columnVisibility) {
                SidebarView(selection: $selectedItem)
            } detail: {
                NavigationStack {
                    DetailRouter(selectedItem: selectedItem)
                }
            }

            // Full window takeover when playing
            if playerModel.isActive {
                PlayerScreen(
                    playerModel: playerModel,
                    columnVisibility: $columnVisibility
                )
                .transition(.opacity)
            }
        }
        .toolbar(playerModel.isActive ? .hidden : .automatic, for: .windowToolbar)
        .environment(playerModel)
        .onAppear {
            playerModel.appState = appState
        }
        .onKeyPress(.escape) {
            if playerModel.isActive {
                // If in fullscreen, exit fullscreen first (match Infuse behavior)
                if let window = NSApp.keyWindow, window.styleMask.contains(.fullScreen) {
                    window.toggleFullScreen(nil)
                    return .handled
                }
                // Otherwise, stop the player
                playerModel.stop()
                columnVisibility = .all
                return .handled
            }
            return .ignored
        }
    }
}

struct DetailRouter: View {
    let selectedItem: SidebarItem?
    @Environment(AppState.self) private var appState

    var body: some View {
        Group {
            switch selectedItem {
            case .home:
                HomeView()
            case .movies:
                MediaGridView(mediaType: .movie, title: "Movies")
            case .tvShows:
                MediaGridView(mediaType: .show, title: "TV Shows")
            case .other:
                MediaGridView(mediaType: .other, title: "Other")
            case .favorites:
                FavoritesView()
            case .files:
                FilesView()
            case .downloads:
                DownloadsView()
            case .settings:
                SettingsView()
            case nil:
                HomeView()
            }
        }
    }
}
