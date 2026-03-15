import AppKit

/// Reusable placeholder view controller for sidebar views not yet implemented.
class PlaceholderViewController: NSViewController {
    private let titleText: String
    private let descriptionText: String
    private let symbolName: String

    init(title: String, description: String, symbol: String) {
        self.titleText = title
        self.descriptionText = description
        self.symbolName = symbol
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) not implemented")
    }

    override func loadView() {
        let container = NSView()
        container.wantsLayer = true

        let stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .centerX
        stack.spacing = 12
        stack.translatesAutoresizingMaskIntoConstraints = false

        // Icon
        let imageView = NSImageView()
        if #available(macOS 11.0, *) {
            let config = NSImage.SymbolConfiguration(pointSize: 48, weight: .light)
            imageView.image = NSImage(systemSymbolName: symbolName, accessibilityDescription: titleText)?
                .withSymbolConfiguration(config)
        }
        imageView.imageScaling = .scaleProportionallyDown
        imageView.contentTintColor = .tertiaryLabelColor
        imageView.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            imageView.widthAnchor.constraint(equalToConstant: 64),
            imageView.heightAnchor.constraint(equalToConstant: 64),
        ])

        // Title
        let titleLabel = NSTextField(labelWithString: titleText)
        titleLabel.font = .systemFont(ofSize: 20, weight: .semibold)
        titleLabel.textColor = .labelColor
        titleLabel.alignment = .center

        // Description
        let descLabel = NSTextField(labelWithString: descriptionText)
        descLabel.font = .systemFont(ofSize: 13)
        descLabel.textColor = .secondaryLabelColor
        descLabel.alignment = .center
        descLabel.maximumNumberOfLines = 3
        descLabel.preferredMaxLayoutWidth = 300

        stack.addArrangedSubview(imageView)
        stack.addArrangedSubview(titleLabel)
        stack.addArrangedSubview(descLabel)

        container.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.centerXAnchor.constraint(equalTo: container.centerXAnchor),
            stack.centerYAnchor.constraint(equalTo: container.centerYAnchor),
        ])

        self.view = container
    }
}

// MARK: - Factory

extension PlaceholderViewController {
    static func home() -> PlaceholderViewController {
        PlaceholderViewController(
            title: "Welcome to Reel",
            description: "Connect to Plex or add local folders in Settings to get started.",
            symbol: "house"
        )
    }

    static func movies() -> PlaceholderViewController {
        PlaceholderViewController(
            title: "Movies",
            description: "Your movie library will appear here.",
            symbol: "film"
        )
    }

    static func tvShows() -> PlaceholderViewController {
        PlaceholderViewController(
            title: "TV Shows",
            description: "Your TV show library will appear here.",
            symbol: "tv"
        )
    }

    static func other() -> PlaceholderViewController {
        PlaceholderViewController(
            title: "Other",
            description: "Other media files will appear here.",
            symbol: "folder"
        )
    }

    static func favorites() -> PlaceholderViewController {
        PlaceholderViewController(
            title: "Favorites",
            description: "No favorites yet \u{2014} right-click any item to add it.",
            symbol: "star.fill"
        )
    }

    static func files() -> PlaceholderViewController {
        PlaceholderViewController(
            title: "Files",
            description: "Connected sources will appear here. Add servers or folders in Settings.",
            symbol: "externaldrive"
        )
    }

    static func downloads() -> PlaceholderViewController {
        PlaceholderViewController(
            title: "Downloads",
            description: "No downloads \u{2014} download Plex items for offline viewing.",
            symbol: "arrow.down.circle"
        )
    }

    static func settings() -> PlaceholderViewController {
        PlaceholderViewController(
            title: "Settings",
            description: "Configure Plex, library folders, and preferences.",
            symbol: "gearshape"
        )
    }
}
