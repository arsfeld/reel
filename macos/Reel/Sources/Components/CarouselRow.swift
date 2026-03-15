import SwiftUI

struct CarouselRow: View {
    let title: String
    let items: [MediaItem]

    var body: some View {
        if !items.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                Text(title)
                    .font(.title3)
                    .fontWeight(.semibold)
                    .padding(.horizontal)

                ScrollView(.horizontal, showsIndicators: false) {
                    LazyHStack(spacing: 12) {
                        ForEach(items) { item in
                            NavigationLink(value: item) {
                                PosterCard(item, width: 130)
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    .padding(.horizontal)
                }
            }
        }
    }
}
