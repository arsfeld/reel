import SwiftUI

struct HomeView: View {
    @Environment(AppState.self) private var appState
    @State private var libraryModel = LibraryModel()
    @State private var heroItem: MediaItem?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                // Hero banner
                if let hero = heroItem {
                    heroBanner(hero)
                }

                // Carousels
                if !libraryModel.continueWatching.isEmpty {
                    CarouselRow(title: "Continue Watching", items: libraryModel.continueWatching)
                }

                CarouselRow(title: "Recently Added", items: libraryModel.recentlyAdded)

                // Genre rows
                ForEach(libraryModel.genres.prefix(8), id: \.self) { genre in
                    if let items = libraryModel.genreItems[genre], !items.isEmpty {
                        CarouselRow(title: genre, items: items)
                    }
                }
            }
            .padding(.vertical)
        }
        .navigationTitle("Home")
        .toolbar {
            if appState.isSyncing {
                ToolbarItem(placement: .automatic) {
                    HStack(spacing: 6) {
                        ProgressView()
                            .controlSize(.small)
                        Text(appState.syncStatus)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
            }
        }
        .navigationDestination(for: MediaItem.self) { item in
            DetailView(item: item)
        }
        .overlay {
            if appState.isSyncing {
                syncingOverlay
            } else if libraryModel.recentlyAdded.isEmpty && libraryModel.continueWatching.isEmpty {
                emptyState
            }
        }
        .onAppear {
            libraryModel.setLibrary(appState.library)
            libraryModel.loadHomeData()
            heroItem = libraryModel.recentlyAdded.first ?? libraryModel.continueWatching.first
        }
        .onChange(of: libraryModel.recentlyAdded) {
            if heroItem == nil {
                heroItem = libraryModel.recentlyAdded.first
            }
        }
        .onChange(of: appState.lastSyncTime) {
            libraryModel.loadHomeData()
        }
    }

    @ViewBuilder
    private func heroBanner(_ item: MediaItem) -> some View {
        ZStack(alignment: .bottomLeading) {
            // Backdrop
            if let path = item.backdropPath, !path.isEmpty {
                AsyncImage(url: URL(fileURLWithPath: path)) { phase in
                    if case .success(let image) = phase {
                        image.resizable()
                            .aspectRatio(contentMode: .fill)
                            .frame(height: 400)
                            .clipped()
                    }
                }
            }

            // Gradient overlay
            LinearGradient(
                colors: [.clear, .black.opacity(0.8)],
                startPoint: .top,
                endPoint: .bottom
            )

            // Title overlay
            VStack(alignment: .leading, spacing: 8) {
                Text(item.title)
                    .font(.largeTitle)
                    .fontWeight(.bold)
                if item.year > 0 {
                    Text(item.yearString)
                        .font(.title3)
                        .foregroundStyle(.secondary)
                }
                if let summary = item.summary {
                    Text(summary)
                        .font(.body)
                        .lineLimit(2)
                        .foregroundStyle(.secondary)
                }
            }
            .padding(24)
        }
        .frame(height: 400)
        .clipShape(RoundedRectangle(cornerRadius: 12))
        .padding(.horizontal)
    }

    private var syncingOverlay: some View {
        VStack(spacing: 12) {
            ProgressView()
                .controlSize(.large)
            Text(appState.syncStatus)
                .font(.headline)
                .foregroundStyle(.secondary)
        }
    }

    private var emptyState: some View {
        ContentUnavailableView {
            Label("Welcome to Reel", systemImage: "film.stack")
        } description: {
            Text("Connect to Plex or add local folders in Settings to get started.")
        } actions: {
            // Navigation to settings handled by sidebar
        }
    }
}
