import SwiftUI

struct MediaGridView: View {
    let mediaType: MediaType
    let title: String

    @Environment(AppState.self) private var appState
    @State private var items: [MediaItem] = []
    @State private var sortBy: SortField = .title
    @State private var sortOrder: SortOrder = .asc
    @State private var isLoading = false
    @State private var offset: Int32 = 0
    private let pageSize: Int32 = 50

    private let columns = [
        GridItem(.adaptive(minimum: 150, maximum: 180), spacing: 16)
    ]

    var body: some View {
        ScrollView {
            LazyVGrid(columns: columns, spacing: 20) {
                ForEach(items) { item in
                    NavigationLink(value: item) {
                        PosterCard(item)
                    }
                    .buttonStyle(.plain)
                    .onAppear {
                        if item.id == items.last?.id {
                            loadMore()
                        }
                    }
                }
            }
            .padding()
        }
        .navigationTitle(title)
        .navigationDestination(for: MediaItem.self) { item in
            DetailView(item: item)
        }
        .toolbar {
            ToolbarItem(placement: .automatic) {
                Menu {
                    ForEach(SortField.allCases, id: \.self) { field in
                        Button {
                            if sortBy == field {
                                sortOrder = sortOrder == .asc ? .desc : .asc
                            } else {
                                sortBy = field
                                sortOrder = .asc
                            }
                            reload()
                        } label: {
                            HStack {
                                Text(field.displayName)
                                if sortBy == field {
                                    Image(systemName: sortOrder == .asc ? "chevron.up" : "chevron.down")
                                }
                            }
                        }
                    }
                } label: {
                    Label("Sort", systemImage: "arrow.up.arrow.down")
                }
            }
        }
        .overlay {
            if items.isEmpty && !isLoading {
                ContentUnavailableView {
                    Label("No \(title)", systemImage: "film")
                } description: {
                    Text("Add media to your library to see it here.")
                }
            }
        }
        .onAppear { reload() }
        .onChange(of: appState.lastSyncTime) { reload() }
    }

    private func reload() {
        guard let lib = appState.library else { return }
        isLoading = true
        offset = 0
        items = ReelBridge.getItems(
            library: lib, type: mediaType,
            sortBy: sortBy, sortOrder: sortOrder,
            limit: pageSize, offset: 0
        )
        offset = Int32(items.count)
        isLoading = false
    }

    private func loadMore() {
        guard let lib = appState.library, !isLoading else { return }
        isLoading = true
        let newItems = ReelBridge.getItems(
            library: lib, type: mediaType,
            sortBy: sortBy, sortOrder: sortOrder,
            limit: pageSize, offset: offset
        )
        if !newItems.isEmpty {
            items.append(contentsOf: newItems)
            offset += Int32(newItems.count)
        }
        isLoading = false
    }
}
