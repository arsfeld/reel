import SwiftUI
import ReelCore

struct DetailView: View {
    let item: MediaItem

    @Environment(AppState.self) private var appState
    @Environment(PlayerModel.self) private var playerModel
    @State private var seasons: [MediaItem] = []
    @State private var episodes: [MediaItem] = []
    @State private var selectedSeason: MediaItem?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                // Cinematic backdrop
                backdrop

                // Metadata
                VStack(alignment: .leading, spacing: 16) {
                    // Title + metadata line
                    VStack(alignment: .leading, spacing: 8) {
                        Text(item.title)
                            .font(.largeTitle)
                            .fontWeight(.bold)

                        HStack(spacing: 12) {
                            if item.year > 0 {
                                Text(item.yearString)
                            }
                            if item.rating > 0 {
                                HStack(spacing: 2) {
                                    Image(systemName: "star.fill")
                                        .foregroundStyle(.yellow)
                                    Text(String(format: "%.1f", item.rating))
                                }
                            }
                            if item.durationMs > 0 {
                                Text(item.formattedDuration)
                            }
                        }
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                    }

                    // Action buttons
                    HStack(spacing: 12) {
                        if let filePath = item.filePath {
                            Button {
                                playerModel.play(filePath: filePath)
                            } label: {
                                Label("Play", systemImage: "play.fill")
                            }
                            .buttonStyle(.borderedProminent)
                        }

                        Button {
                            toggleFavorite()
                        } label: {
                            Label("Favorite", systemImage: "star")
                        }

                        Button {
                            toggleWatched()
                        } label: {
                            Label("Watched", systemImage: "checkmark.circle")
                        }
                    }

                    // Summary
                    if let summary = item.summary, !summary.isEmpty {
                        Text(summary)
                            .font(.body)
                            .foregroundStyle(.secondary)
                            .lineLimit(4)
                    }

                    // TV Show drill-down
                    if item.mediaType == .show && !seasons.isEmpty {
                        tvShowContent
                    }
                }
                .padding(24)
            }
        }
        .navigationTitle(item.title)
        .onAppear { loadChildren() }
    }

    @ViewBuilder
    private var backdrop: some View {
        ZStack(alignment: .bottom) {
            if let path = item.backdropPath, !path.isEmpty {
                AsyncImage(url: URL(fileURLWithPath: path)) { phase in
                    if case .success(let image) = phase {
                        image.resizable()
                            .aspectRatio(contentMode: .fill)
                            .frame(height: 350)
                            .clipped()
                    } else {
                        Color.black.frame(height: 350)
                    }
                }
            } else {
                Color.black.frame(height: 350)
            }

            LinearGradient(
                colors: [.clear, Color(.windowBackgroundColor)],
                startPoint: .center,
                endPoint: .bottom
            )
            .frame(height: 200)
        }
        .frame(height: 350)
    }

    @ViewBuilder
    private var tvShowContent: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Seasons")
                .font(.title2)
                .fontWeight(.semibold)

            // Season picker
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 8) {
                    ForEach(seasons) { season in
                        Button {
                            selectedSeason = season
                            loadEpisodes(for: season)
                        } label: {
                            Text("Season \(season.seasonNumber)")
                                .padding(.horizontal, 12)
                                .padding(.vertical, 6)
                        }
                        .buttonStyle(.bordered)
                        .tint(selectedSeason?.id == season.id ? .accentColor : .secondary)
                    }
                }
            }

            // Episode list
            if !episodes.isEmpty {
                ForEach(episodes) { episode in
                    HStack(spacing: 12) {
                        Text("E\(episode.episodeNumber)")
                            .font(.caption)
                            .fontWeight(.bold)
                            .foregroundStyle(.secondary)
                            .frame(width: 30)

                        VStack(alignment: .leading, spacing: 2) {
                            Text(episode.title)
                                .font(.body)
                                .lineLimit(1)
                            if episode.durationMs > 0 {
                                Text(episode.formattedDuration)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }

                        Spacer()

                        if let filePath = episode.filePath {
                            Button {
                                playerModel.play(filePath: filePath)
                            } label: {
                                Image(systemName: "play.circle.fill")
                                    .font(.title2)
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    .padding(.vertical, 4)
                    Divider()
                }
            }
        }
    }

    private func loadChildren() {
        guard let lib = appState.library, item.mediaType == .show else { return }
        seasons = ReelBridge.getItemsByParent(library: lib, parentId: item.id)
            .filter { $0.mediaType == .season }
            .sorted { $0.seasonNumber < $1.seasonNumber }
        if let first = seasons.first {
            selectedSeason = first
            loadEpisodes(for: first)
        }
    }

    private func loadEpisodes(for season: MediaItem) {
        guard let lib = appState.library else { return }
        episodes = ReelBridge.getItemsByParent(library: lib, parentId: season.id)
            .filter { $0.mediaType == .episode }
            .sorted { $0.episodeNumber < $1.episodeNumber }
    }

    private func toggleFavorite() {
        guard let lib = appState.library else { return }
        reel_library_add_favorite(lib, "media_item", String(item.id), item.title)
    }

    private func toggleWatched() {
        guard let lib = appState.library else { return }
        ReelBridge.updateWatchProgress(
            library: lib, mediaItemId: item.id,
            positionMs: item.durationMs, durationMs: item.durationMs,
            watched: true
        )
    }
}
