import SwiftUI

struct PosterCard: View {
    let item: MediaItem
    let width: CGFloat

    init(_ item: MediaItem, width: CGFloat = 150) {
        self.item = item
        self.width = width
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            posterImage
                .frame(width: width, height: width * 1.5)
                .clipShape(RoundedRectangle(cornerRadius: 8))

            Text(item.title)
                .font(.caption)
                .fontWeight(.medium)
                .lineLimit(1)

            if item.year > 0 {
                Text(item.yearString)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        }
        .frame(width: width)
    }

    @ViewBuilder
    private var posterImage: some View {
        if let url = item.posterURL {
            AsyncImage(url: url) { phase in
                switch phase {
                case .success(let image):
                    image.resizable().aspectRatio(contentMode: .fill)
                case .failure:
                    posterPlaceholder
                default:
                    posterPlaceholder.overlay {
                        ProgressView()
                    }
                }
            }
        } else {
            posterPlaceholder
        }
    }

    private var posterPlaceholder: some View {
        ZStack {
            LinearGradient(
                colors: [.blue.opacity(0.3), .purple.opacity(0.3)],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
            VStack(spacing: 4) {
                Image(systemName: item.mediaType == .show ? "tv" : "film")
                    .font(.title2)
                Text(item.title)
                    .font(.caption2)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 4)
            }
            .foregroundStyle(.secondary)
        }
    }
}
