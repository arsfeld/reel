import SwiftUI
import ReelCore

@Observable
final class LibraryModel {
    var recentlyAdded: [MediaItem] = []
    var continueWatching: [MediaItem] = []
    var favorites: [ReelBridge.Favorite] = []
    var genres: [String] = []
    var genreItems: [String: [MediaItem]] = [:]

    private var library: OpaquePointer?

    func setLibrary(_ lib: OpaquePointer?) {
        self.library = lib
    }

    func loadHomeData() {
        guard let lib = library else { return }
        Task.detached { [weak self] in
            let recent = ReelBridge.getRecentlyAdded(library: lib, limit: 20)
            let watching = ReelBridge.getContinueWatching(library: lib, limit: 20)
            let favs = ReelBridge.listFavorites(library: lib)
            let genreList = ReelBridge.getGenres(library: lib)

            var genreItemsMap: [String: [MediaItem]] = [:]
            for genre in genreList.prefix(10) {
                genreItemsMap[genre] = ReelBridge.getItemsByGenre(library: lib, genre: genre, limit: 20)
            }

            await MainActor.run {
                self?.recentlyAdded = recent
                self?.continueWatching = watching
                self?.favorites = favs
                self?.genres = genreList
                self?.genreItems = genreItemsMap
            }
        }
    }

    func getItems(
        type mediaType: MediaType,
        sortBy: SortField = .title,
        sortOrder: SortOrder = .asc,
        limit: Int32 = 50,
        offset: Int32 = 0
    ) -> [MediaItem] {
        guard let lib = library else { return [] }
        return ReelBridge.getItems(library: lib, type: mediaType, sortBy: sortBy, sortOrder: sortOrder, limit: limit, offset: offset)
    }

    func search(query: String) -> [MediaItem] {
        guard let lib = library else { return [] }
        return ReelBridge.search(library: lib, query: query)
    }

    func getItem(id: Int64) -> MediaItem? {
        guard let lib = library else { return nil }
        return ReelBridge.getItem(library: lib, id: id)
    }

    func getChildren(parentId: Int64) -> [MediaItem] {
        guard let lib = library else { return [] }
        return ReelBridge.getItemsByParent(library: lib, parentId: parentId)
    }
}
