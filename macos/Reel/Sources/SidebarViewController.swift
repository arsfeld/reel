import AppKit

enum SidebarItem: String, CaseIterable {
    case home = "Home"
    case movies = "Movies"
    case tvShows = "TV Shows"
    case other = "Other"
    case favorites = "Favorites"
    case files = "Files"
    case downloads = "Downloads"
    case settings = "Settings"

    var icon: NSImage.Name {
        switch self {
        case .home: return NSImage.homeTemplateName
        case .movies: return "camera.fill"
        case .tvShows: return "tv"
        case .other: return "film"
        case .favorites: return "star.fill"
        case .files: return "externaldrive.connected.to.line.below"
        case .downloads: return "arrow.down.circle"
        case .settings: return NSImage.actionTemplateName
        }
    }

    var systemSymbol: String {
        switch self {
        case .home: return "house"
        case .movies: return "film"
        case .tvShows: return "tv"
        case .other: return "folder"
        case .favorites: return "star.fill"
        case .files: return "externaldrive"
        case .downloads: return "arrow.down.circle"
        case .settings: return "gearshape"
        }
    }
}

struct SidebarSection {
    let title: String
    let items: [SidebarItem]
}

let sidebarSections = [
    SidebarSection(title: "Library", items: [.home, .movies, .tvShows, .other]),
    SidebarSection(title: "Sources", items: [.favorites, .files]),
    SidebarSection(title: "", items: [.downloads, .settings]),
]

protocol SidebarDelegate: AnyObject {
    func sidebarDidSelectItem(_ item: SidebarItem)
}

class SidebarViewController: NSViewController, NSOutlineViewDataSource, NSOutlineViewDelegate {
    weak var delegate: SidebarDelegate?
    private var outlineView: NSOutlineView!

    override func loadView() {
        let scrollView = NSScrollView()
        scrollView.hasVerticalScroller = true
        scrollView.autohidesScrollers = true

        outlineView = NSOutlineView()
        outlineView.headerView = nil
        outlineView.indentationPerLevel = 0
        outlineView.rowSizeStyle = .medium
        outlineView.selectionHighlightStyle = .sourceList
        outlineView.dataSource = self
        outlineView.delegate = self
        outlineView.target = self
        outlineView.action = #selector(outlineViewClicked(_:))

        let column = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("SidebarColumn"))
        column.isEditable = false
        outlineView.addTableColumn(column)
        outlineView.outlineTableColumn = column

        scrollView.documentView = outlineView
        self.view = scrollView
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        // Defer expansion to avoid deadlock during contentViewController assignment
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            // Expand all sections
            for section in sidebarSections {
                self.outlineView.expandItem(section.title)
            }
            // Select Home by default
            let homeRow = self.outlineView.row(forItem: SidebarItem.home)
            if homeRow >= 0 {
                self.outlineView.selectRowIndexes(IndexSet(integer: homeRow), byExtendingSelection: false)
            }
        }
    }

    // MARK: - NSOutlineViewDataSource

    func outlineView(_ outlineView: NSOutlineView, numberOfChildrenOfItem item: Any?) -> Int {
        if item == nil {
            return sidebarSections.count
        }
        if let title = item as? String {
            return sidebarSections.first { $0.title == title }?.items.count ?? 0
        }
        return 0
    }

    func outlineView(_ outlineView: NSOutlineView, child index: Int, ofItem item: Any?) -> Any {
        if item == nil {
            return sidebarSections[index].title
        }
        if let title = item as? String {
            return sidebarSections.first { $0.title == title }?.items[index] ?? SidebarItem.home
        }
        return SidebarItem.home
    }

    func outlineView(_ outlineView: NSOutlineView, isItemExpandable item: Any) -> Bool {
        return item is String
    }

    // MARK: - NSOutlineViewDelegate

    func outlineView(_ outlineView: NSOutlineView, viewFor tableColumn: NSTableColumn?, item: Any) -> NSView? {
        if let title = item as? String {
            let cell = NSTextField(labelWithString: title.uppercased())
            cell.font = .systemFont(ofSize: 11, weight: .semibold)
            cell.textColor = .secondaryLabelColor
            return cell
        }

        if let sidebarItem = item as? SidebarItem {
            let cellView = NSTableCellView()
            cellView.identifier = NSUserInterfaceItemIdentifier("SidebarCell")

            let imageView = NSImageView()
            if #available(macOS 11.0, *) {
                imageView.image = NSImage(systemSymbolName: sidebarItem.systemSymbol, accessibilityDescription: sidebarItem.rawValue)
            } else {
                imageView.image = NSImage(named: sidebarItem.icon)
            }
            imageView.imageScaling = .scaleProportionallyDown
            imageView.translatesAutoresizingMaskIntoConstraints = false
            imageView.setContentHuggingPriority(.required, for: .horizontal)

            let textField = NSTextField(labelWithString: sidebarItem.rawValue)
            textField.translatesAutoresizingMaskIntoConstraints = false

            cellView.addSubview(imageView)
            cellView.addSubview(textField)
            cellView.imageView = imageView
            cellView.textField = textField

            NSLayoutConstraint.activate([
                imageView.leadingAnchor.constraint(equalTo: cellView.leadingAnchor, constant: 4),
                imageView.centerYAnchor.constraint(equalTo: cellView.centerYAnchor),
                imageView.widthAnchor.constraint(equalToConstant: 18),
                imageView.heightAnchor.constraint(equalToConstant: 18),
                textField.leadingAnchor.constraint(equalTo: imageView.trailingAnchor, constant: 6),
                textField.trailingAnchor.constraint(lessThanOrEqualTo: cellView.trailingAnchor, constant: -4),
                textField.centerYAnchor.constraint(equalTo: cellView.centerYAnchor),
            ])

            return cellView
        }

        return nil
    }

    func outlineView(_ outlineView: NSOutlineView, isGroupItem item: Any) -> Bool {
        return item is String
    }

    func outlineView(_ outlineView: NSOutlineView, shouldSelectItem item: Any) -> Bool {
        return item is SidebarItem
    }

    func outlineView(_ outlineView: NSOutlineView, shouldShowOutlineCellForItem item: Any) -> Bool {
        return false
    }

    @objc private func outlineViewClicked(_ sender: NSOutlineView) {
        let row = sender.clickedRow
        guard row >= 0, let item = sender.item(atRow: row) as? SidebarItem else { return }
        delegate?.sidebarDidSelectItem(item)
    }

    func outlineViewSelectionDidChange(_ notification: Notification) {
        let row = outlineView.selectedRow
        guard row >= 0, let item = outlineView.item(atRow: row) as? SidebarItem else { return }
        delegate?.sidebarDidSelectItem(item)
    }
}
