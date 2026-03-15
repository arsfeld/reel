import SwiftUI

struct FilesView: View {
    @Environment(AppState.self) private var appState
    @State private var servers: [ReelBridge.Server] = []
    @State private var scanPaths: [ReelBridge.ScanPath] = []

    var body: some View {
        List {
            if !servers.isEmpty {
                Section("Plex Servers") {
                    ForEach(servers) { server in
                        HStack {
                            Image(systemName: "server.rack")
                            VStack(alignment: .leading) {
                                Text(server.name)
                                    .font(.body)
                                Text(server.connectionUri)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                }
            }

            if !scanPaths.isEmpty {
                Section("Local Folders") {
                    ForEach(scanPaths) { path in
                        HStack {
                            Image(systemName: "folder")
                            Text(path.path)
                        }
                    }
                }
            }
        }
        .navigationTitle("Files")
        .overlay {
            if servers.isEmpty && scanPaths.isEmpty {
                ContentUnavailableView {
                    Label("No Sources", systemImage: "externaldrive")
                } description: {
                    Text("Connect to Plex or add local folders in Settings.")
                }
            }
        }
        .onAppear { reload() }
    }

    private func reload() {
        guard let lib = appState.library else { return }
        servers = ReelBridge.listServers(library: lib)
        scanPaths = ReelBridge.listScanPaths(library: lib)
    }
}
