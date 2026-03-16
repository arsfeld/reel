import SwiftUI

struct PosterCard: View {
    let item: MediaItem
    let width: CGFloat
    let showPlayButton: Bool

    @Environment(PlayerModel.self) private var playerModel
    @State private var isHovered = false

    init(_ item: MediaItem, width: CGFloat = 150, showPlayButton: Bool = true) {
        self.item = item
        self.width = width
        self.showPlayButton = showPlayButton
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            ZStack {
                posterImage
                    .frame(width: width, height: width * 1.5)
                    .clipShape(RoundedRectangle(cornerRadius: 8))

                // Play button overlay on hover for playable items
                if showPlayButton && item.filePath != nil && isHovered {
                    RoundedRectangle(cornerRadius: 8)
                        .fill(.black.opacity(0.4))
                        .frame(width: width, height: width * 1.5)

                    Button {
                        if let path = item.filePath {
                            playerModel.play(filePath: path, mediaItemId: item.id)
                        }
                    } label: {
                        Image(systemName: "play.circle.fill")
                            .font(.system(size: 44))
                            .foregroundStyle(.white)
                            .shadow(radius: 4)
                    }
                    .buttonStyle(.plain)
                }
            }
            .onHover { hovering in
                withAnimation(.easeInOut(duration: 0.15)) {
                    isHovered = hovering
                }
            }

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
