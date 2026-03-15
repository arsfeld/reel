import SwiftUI
import ReelCore

struct DownloadsView: View {
    @Environment(AppState.self) private var appState
    @State private var downloads: [ReelBridge.Download] = []
    @State private var pollTimer: Timer?

    var body: some View {
        List {
            ForEach(downloads) { download in
                HStack(spacing: 12) {
                    statusIcon(download.status)

                    VStack(alignment: .leading, spacing: 4) {
                        Text("Item #\(download.mediaItemId)")
                            .font(.body)

                        if download.totalBytes > 0 {
                            ProgressView(value: download.progress)
                            Text(formatBytes(download.downloadedBytes) + " / " + formatBytes(download.totalBytes))
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        } else {
                            Text(download.status.capitalized)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }

                        if let error = download.errorMessage {
                            Text(error)
                                .font(.caption)
                                .foregroundStyle(.red)
                        }
                    }

                    Spacer()

                    // Action buttons
                    HStack(spacing: 8) {
                        if download.status == "downloading" || download.status == "queued" {
                            Button {
                                guard let dl = appState.downloader else { return }
                                reel_download_pause(dl, download.id)
                                reload()
                            } label: {
                                Image(systemName: "pause.circle")
                            }
                            .buttonStyle(.plain)
                        }

                        if download.status == "paused" {
                            Button {
                                guard let dl = appState.downloader else { return }
                                reel_download_resume(dl, download.id)
                                reload()
                            } label: {
                                Image(systemName: "play.circle")
                            }
                            .buttonStyle(.plain)
                        }

                        Button {
                            guard let dl = appState.downloader else { return }
                            reel_download_remove(dl, download.id, 0)
                            reload()
                        } label: {
                            Image(systemName: "trash")
                                .foregroundStyle(.red)
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(.vertical, 4)
            }
        }
        .navigationTitle("Downloads")
        .overlay {
            if downloads.isEmpty {
                ContentUnavailableView {
                    Label("No Downloads", systemImage: "arrow.down.circle")
                } description: {
                    Text("Download Plex items for offline viewing.")
                }
            }
        }
        .onAppear {
            reload()
            pollTimer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { _ in
                reload()
            }
        }
        .onDisappear {
            pollTimer?.invalidate()
            pollTimer = nil
        }
    }

    private func reload() {
        guard let dl = appState.downloader else { return }
        downloads = ReelBridge.listDownloads(downloader: dl)
    }

    private func statusIcon(_ status: String) -> some View {
        let (icon, color): (String, Color) = switch status {
        case "downloading": ("arrow.down.circle.fill", .blue)
        case "paused": ("pause.circle.fill", .orange)
        case "complete": ("checkmark.circle.fill", .green)
        case "failed": ("xmark.circle.fill", .red)
        default: ("clock.fill", .secondary)
        }
        return Image(systemName: icon)
            .foregroundStyle(color)
            .font(.title2)
    }

    private func formatBytes(_ bytes: Int64) -> String {
        let formatter = ByteCountFormatter()
        formatter.countStyle = .file
        return formatter.string(fromByteCount: bytes)
    }
}
