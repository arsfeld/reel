import SwiftUI

struct ContentView: View {
    @EnvironmentObject var appModel: AppModel
    @State private var selectedBackend: String?
    
    var body: some View {
        NavigationSplitView {
            SidebarView(selection: $selectedBackend)
        } detail: {
            if let backend = selectedBackend {
                LibraryView(backendId: backend)
            } else {
                WelcomeView()
            }
        }
        .navigationTitle("Reel")
        .toolbar {
            ToolbarItem(placement: .navigation) {
                Button(action: toggleSidebar) {
                    Image(systemName: "sidebar.left")
                }
            }
            
            ToolbarItem(placement: .primaryAction) {
                Button(action: { appModel.refreshLibraries() }) {
                    Image(systemName: "arrow.clockwise")
                }
                .disabled(!appModel.isInitialized)
            }
        }
    }
    
    private func toggleSidebar() {
        NSApp.keyWindow?.firstResponder?.tryToPerform(
            #selector(NSSplitViewController.toggleSidebar(_:)),
            with: nil
        )
    }
}

struct WelcomeView: View {
    var body: some View {
        VStack(spacing: 20) {
            Image(systemName: "film.stack")
                .font(.system(size: 72))
                .foregroundColor(.secondary)
            
            Text("Welcome to Reel")
                .font(.largeTitle)
                .fontWeight(.bold)
            
            Text("Connect your media servers to get started")
                .font(.title3)
                .foregroundColor(.secondary)
            
            Button("Add Source") {
                // TODO: Show add source sheet
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

struct SidebarView: View {
    @EnvironmentObject var appModel: AppModel
    @Binding var selection: String?
    
    var body: some View {
        List(selection: $selection) {
            Section("Sources") {
                ForEach(appModel.backends, id: \.id) { backend in
                    Label(backend.name, systemImage: iconForBackend(backend.type))
                        .tag(backend.id)
                }
            }
            
            Section("Libraries") {
                ForEach(appModel.libraries, id: \.id) { library in
                    Label(library.name, systemImage: iconForLibraryType(library.type))
                        .tag(library.id)
                }
            }
        }
        .listStyle(.sidebar)
        .navigationTitle("Reel")
    }
    
    private func iconForBackend(_ type: String) -> String {
        switch type {
        case "plex": return "server.rack"
        case "jellyfin": return "play.square.stack"
        case "local": return "folder"
        default: return "questionmark.circle"
        }
    }
    
    private func iconForLibraryType(_ type: String) -> String {
        switch type {
        case "movie": return "film"
        case "show": return "tv"
        case "music": return "music.note"
        default: return "folder"
        }
    }
}

struct LibraryView: View {
    let backendId: String
    @EnvironmentObject var appModel: AppModel
    
    var body: some View {
        ScrollView {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 150))], spacing: 20) {
                ForEach(appModel.getMediaItems(for: backendId), id: \.id) { item in
                    MediaItemView(item: item)
                }
            }
            .padding()
        }
        .navigationTitle(appModel.getBackend(id: backendId)?.name ?? "Library")
    }
}

struct MediaItemView: View {
    let item: MediaItem
    
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            RoundedRectangle(cornerRadius: 8)
                .fill(Color.gray.opacity(0.3))
                .aspectRatio(2/3, contentMode: .fit)
                .overlay(
                    Image(systemName: "photo")
                        .font(.largeTitle)
                        .foregroundColor(.secondary)
                )
            
            Text(item.title)
                .font(.caption)
                .lineLimit(2)
            
            Text(item.year)
                .font(.caption2)
                .foregroundColor(.secondary)
        }
        .frame(width: 150)
    }
}

struct SettingsView: View {
    @EnvironmentObject var appModel: AppModel
    
    var body: some View {
        TabView {
            GeneralSettingsView()
                .tabItem {
                    Label("General", systemImage: "gear")
                }
            
            SourcesSettingsView()
                .tabItem {
                    Label("Sources", systemImage: "server.rack")
                }
        }
        .frame(width: 500, height: 400)
    }
}

struct GeneralSettingsView: View {
    var body: some View {
        Form {
            Text("General settings will go here")
        }
        .padding()
    }
}

struct SourcesSettingsView: View {
    @EnvironmentObject var appModel: AppModel
    
    var body: some View {
        VStack {
            List(appModel.backends, id: \.id) { backend in
                HStack {
                    VStack(alignment: .leading) {
                        Text(backend.name)
                            .font(.headline)
                        Text(backend.type)
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                    Spacer()
                    Button("Remove") {
                        // TODO: Remove backend
                    }
                    .buttonStyle(.borderless)
                }
            }
            
            HStack {
                Button("Add Source") {
                    // TODO: Show add source sheet
                }
                Spacer()
            }
            .padding()
        }
    }
}