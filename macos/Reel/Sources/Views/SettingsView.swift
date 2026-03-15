import SwiftUI
import ReelCore

struct SettingsView: View {
    @Environment(AppState.self) private var appState
    @State private var servers: [ReelBridge.Server] = []
    @State private var scanPaths: [ReelBridge.ScanPath] = []
    @State private var plexState: PlexAuthState = .disconnected
    @State private var plexPin: String = ""
    @State private var authTask: Task<Void, Never>?

    enum PlexAuthState {
        case disconnected
        case waitingForAuth(pin: String)
        case connected
    }

    var body: some View {
        Form {
            // Plex section
            Section("Plex") {
                switch plexState {
                case .disconnected:
                    Button("Connect to Plex") {
                        startPlexAuth()
                    }

                case .waitingForAuth(let pin):
                    VStack(alignment: .leading, spacing: 8) {
                        HStack {
                            ProgressView()
                                .controlSize(.small)
                            Text("Waiting for authentication...")
                        }
                        Text("PIN: \(pin)")
                            .font(.title2.monospaced())
                            .textSelection(.enabled)
                        Text("Enter this code at plex.tv/link")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }

                case .connected:
                    ForEach(servers) { server in
                        HStack {
                            Image(systemName: "checkmark.circle.fill")
                                .foregroundStyle(.green)
                            VStack(alignment: .leading) {
                                Text(server.name)
                                Text(server.connectionUri)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                            Spacer()
                            Button("Disconnect") {
                                disconnect(server: server)
                            }
                            .foregroundStyle(.red)
                        }
                    }
                    Button("Add Another Server") {
                        startPlexAuth()
                    }
                }
            }

            // Library section
            Section("Library") {
                ForEach(scanPaths) { path in
                    HStack {
                        Image(systemName: "folder")
                        Text(path.path)
                        Spacer()
                        Button {
                            removeScanPath(id: path.id)
                        } label: {
                            Image(systemName: "minus.circle")
                                .foregroundStyle(.red)
                        }
                        .buttonStyle(.plain)
                    }
                }
                Button("Add Folder...") {
                    addFolder()
                }
            }
        }
        .formStyle(.grouped)
        .navigationTitle("Settings")
        .onAppear { reload() }
        .onDisappear {
            authTask?.cancel()
        }
    }

    private func reload() {
        guard let lib = appState.library else { return }
        servers = ReelBridge.listServers(library: lib)
        scanPaths = ReelBridge.listScanPaths(library: lib)
        plexState = servers.isEmpty ? .disconnected : .connected
    }

    private func startPlexAuth() {
        guard let db = appState.db else { return }
        guard let plex = reel_plex_create(db) else { return }

        guard let pinPtr = reel_plex_request_pin(plex) else {
            reel_plex_destroy(plex)
            return
        }
        let pin = String(cString: pinPtr)
        plexState = .waitingForAuth(pin: pin)

        // Open browser
        if let urlPtr = reel_plex_get_auth_url(plex) {
            let urlStr = String(cString: urlPtr)
            if let url = URL(string: urlStr) {
                NSWorkspace.shared.open(url)
            }
        }

        // Poll for auth
        authTask = Task {
            let maxAttempts = 150 // 5 minutes at 2s intervals
            for _ in 0..<maxAttempts {
                if Task.isCancelled { break }
                try? await Task.sleep(for: .seconds(2))
                if Task.isCancelled { break }

                if let tokenPtr = reel_plex_poll_pin(plex) {
                    _ = String(cString: tokenPtr)
                    // Discover and save servers
                    var serverBuf = [ReelPlexServerC](repeating: ReelPlexServerC(), count: 10)
                    let count = reel_plex_discover_servers(plex, &serverBuf, 10)
                    if count > 0, let lib = appState.library {
                        for i in 0..<Int(count) {
                            reel_server_upsert(
                                lib,
                                serverBuf[i].id,
                                serverBuf[i].name,
                                serverBuf[i].uri,
                                serverBuf[i].access_token
                            )
                        }
                    }
                    // Sync library from each discovered server
                    if let lib = appState.library {
                        for i in 0..<Int(count) {
                            let srvId = String(cString: serverBuf[i].id)
                            let srvUri = String(cString: serverBuf[i].uri)
                            let srvToken = String(cString: serverBuf[i].access_token)
                            reel_plex_sync(lib, srvId, srvUri, srvToken)
                        }
                    }

                    reel_plex_destroy(plex)
                    await MainActor.run { reload() }
                    return
                }
            }
            // Timeout
            reel_plex_destroy(plex)
            await MainActor.run { plexState = .disconnected }
        }
    }

    private func disconnect(server: ReelBridge.Server) {
        guard let lib = appState.library else { return }
        reel_server_delete(lib, server.id)
        reload()
    }

    private func addFolder() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        if panel.runModal() == .OK, let url = panel.url {
            guard let lib = appState.library else { return }
            reel_library_add_scan_path(lib, url.path)
            reload()
        }
    }

    private func removeScanPath(id: Int64) {
        guard let lib = appState.library else { return }
        reel_library_remove_scan_path(lib, id)
        reload()
    }
}

extension ReelPlexServerC {
    init() {
        self.init(
            id: UnsafePointer(bitPattern: 1)!,
            name: UnsafePointer(bitPattern: 1)!,
            uri: UnsafePointer(bitPattern: 1)!,
            access_token: UnsafePointer(bitPattern: 1)!
        )
    }
}
