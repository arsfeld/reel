import SwiftUI

enum SidebarItem: String, CaseIterable, Identifiable {
    case home
    case movies
    case tvShows
    case other
    case favorites
    case files
    case downloads
    case settings

    var id: String { rawValue }

    var title: String {
        switch self {
        case .home: return "Home"
        case .movies: return "Movies"
        case .tvShows: return "TV Shows"
        case .other: return "Other"
        case .favorites: return "Favorites"
        case .files: return "Files"
        case .downloads: return "Downloads"
        case .settings: return "Settings"
        }
    }

    var icon: String {
        switch self {
        case .home: return "house"
        case .movies: return "film"
        case .tvShows: return "tv"
        case .other: return "tray.full"
        case .favorites: return "star.fill"
        case .files: return "externaldrive"
        case .downloads: return "arrow.down.circle"
        case .settings: return "gearshape"
        }
    }

    enum Section: String, CaseIterable {
        case library = "Library"
        case sources = "Sources"
        case management = "Management"

        var items: [SidebarItem] {
            switch self {
            case .library: return [.home, .movies, .tvShows, .other]
            case .sources: return [.favorites, .files]
            case .management: return [.downloads, .settings]
            }
        }
    }
}

struct SidebarView: View {
    @Binding var selection: SidebarItem?

    var body: some View {
        List(selection: $selection) {
            ForEach(SidebarItem.Section.allCases, id: \.self) { section in
                Section(section.rawValue) {
                    ForEach(section.items) { item in
                        Label(item.title, systemImage: item.icon)
                            .tag(item)
                    }
                }
            }
        }
        .listStyle(.sidebar)
        .navigationTitle("Reel")
    }
}
