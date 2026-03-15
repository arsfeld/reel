import SwiftUI

struct FavoritesView: View {
    @Environment(AppState.self) private var appState
    @State private var favorites: [ReelBridge.Favorite] = []

    var body: some View {
        List {
            ForEach(favorites) { fav in
                HStack {
                    Image(systemName: "star.fill")
                        .foregroundStyle(.yellow)
                    Text(fav.displayName)
                }
            }
        }
        .navigationTitle("Favorites")
        .overlay {
            if favorites.isEmpty {
                ContentUnavailableView {
                    Label("No Favorites", systemImage: "star")
                } description: {
                    Text("Add items to your favorites from the detail view.")
                }
            }
        }
        .onAppear {
            guard let lib = appState.library else { return }
            favorites = ReelBridge.listFavorites(library: lib)
        }
    }
}
