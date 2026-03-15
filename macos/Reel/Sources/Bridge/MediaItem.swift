import Foundation
import ReelCore

enum MediaType: Int, Hashable, Sendable {
    case movie = 0
    case show = 1
    case season = 2
    case episode = 3
    case other = 4

    var reelType: ReelMediaType {
        ReelMediaType(rawValue: UInt32(rawValue))
    }
}

enum MediaSource: Int, Hashable, Sendable {
    case plex = 0
    case local = 1
}

enum SortField: Int32, CaseIterable {
    case title = 0
    case year = 1
    case rating = 2
    case added = 3

    var displayName: String {
        switch self {
        case .title: return "Title"
        case .year: return "Year"
        case .rating: return "Rating"
        case .added: return "Date Added"
        }
    }
}

enum SortOrder: Int32 {
    case asc = 0
    case desc = 1
}

struct MediaItem: Identifiable, Hashable {
    let id: Int64
    let mediaType: MediaType
    let source: MediaSource
    let title: String
    let sortTitle: String?
    let year: Int32
    let summary: String?
    let rating: Double
    let durationMs: Int64
    let posterPath: String?
    let backdropPath: String?
    let parentId: Int64
    let seasonNumber: Int32
    let episodeNumber: Int32
    let filePath: String?

    init(from c: ReelMediaItem) {
        self.id = c.id
        self.mediaType = MediaType(rawValue: Int(c.media_type)) ?? .other
        self.source = MediaSource(rawValue: Int(c.source)) ?? .local
        self.title = String(cString: c.title)
        self.sortTitle = c.sort_title.map { String(cString: $0) }
        self.year = c.year
        self.summary = c.summary.map { String(cString: $0) }
        self.rating = c.rating
        self.durationMs = c.duration_ms
        self.posterPath = c.poster_path.map { String(cString: $0) }
        self.backdropPath = c.backdrop_path.map { String(cString: $0) }
        self.parentId = c.parent_id
        self.seasonNumber = c.season_number
        self.episodeNumber = c.episode_number
        self.filePath = c.file_path.map { String(cString: $0) }
    }

    var formattedDuration: String {
        let totalSeconds = durationMs / 1000
        let hours = totalSeconds / 3600
        let minutes = (totalSeconds % 3600) / 60
        if hours > 0 {
            return "\(hours)h \(minutes)m"
        }
        return "\(minutes)m"
    }

    var yearString: String {
        year > 0 ? String(year) : ""
    }
}
