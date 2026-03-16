import AppKit
import ReelCore

class MainWindow: NSWindowController, SidebarDelegate {
    private var videoView: VideoView!
    private var controlsView: PlayerControlsView!
    private var hideControlsTimer: Timer?
    private var splitViewController: NSSplitViewController!
    private var sidebarViewController: SidebarViewController!
    private var activeContentVC: NSViewController!
    private var isDirectPlay = false
    private var viewControllers: [SidebarItem: NSViewController] = [:]

    convenience init() {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1280, height: 720),
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        window.title = "Reel"
        window.center()
        window.isReleasedWhenClosed = false
        window.minSize = NSSize(width: 800, height: 500)
        window.titlebarAppearsTransparent = false
        window.toolbarStyle = .unified

        self.init(window: window)

        let args = ProcessInfo.processInfo.arguments
        if args.count > 1 {
            isDirectPlay = true
            setupPlayerOnlyLayout()
        } else {
            setupSidebarLayout()
        }

        setupTrackingArea()
    }

    // MARK: - Layout modes

    private func setupSidebarLayout() {
        splitViewController = NSSplitViewController()

        // Sidebar
        sidebarViewController = SidebarViewController()
        sidebarViewController.delegate = self
        let sidebarItem = NSSplitViewItem(sidebarWithViewController: sidebarViewController)
        sidebarItem.minimumThickness = 200
        sidebarItem.maximumThickness = 280
        sidebarItem.canCollapse = true
        splitViewController.addSplitViewItem(sidebarItem)

        // Content
        activeContentVC = PlaceholderViewController.home()
        let contentItem = NSSplitViewItem(viewController: activeContentVC)
        contentItem.minimumThickness = 400
        splitViewController.addSplitViewItem(contentItem)

        // Store initial view controllers
        viewControllers[.home] = activeContentVC

        window?.contentViewController = splitViewController
    }

    private func setupPlayerOnlyLayout() {
        guard let contentView = window?.contentView else { return }

        window?.titlebarAppearsTransparent = true
        window?.titleVisibility = .hidden
        window?.backgroundColor = .black

        // Video view fills the entire window
        videoView = VideoView(frame: contentView.bounds)
        videoView.autoresizingMask = [.width, .height]
        contentView.addSubview(videoView)

        // Controls overlay at the bottom
        let controlsHeight: CGFloat = 80
        controlsView = PlayerControlsView(
            frame: NSRect(
                x: 20,
                y: 20,
                width: contentView.bounds.width - 40,
                height: controlsHeight
            ),
            videoView: videoView
        )
        controlsView.autoresizingMask = [.width, .minYMargin]
        contentView.addSubview(controlsView)
    }

    private func setupTrackingArea() {
        guard let contentView = window?.contentView else { return }
        let trackingArea = NSTrackingArea(
            rect: contentView.bounds,
            options: [.mouseMoved, .activeInKeyWindow, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        contentView.addTrackingArea(trackingArea)
    }

    // MARK: - SidebarDelegate

    func sidebarDidSelectItem(_ item: SidebarItem) {
        let vc = viewControllerForItem(item)

        // Replace the content split view item
        if splitViewController.splitViewItems.count > 1 {
            splitViewController.removeSplitViewItem(splitViewController.splitViewItems[1])
        }

        let contentItem = NSSplitViewItem(viewController: vc)
        contentItem.minimumThickness = 400
        splitViewController.addSplitViewItem(contentItem)

        activeContentVC = vc
    }

    private func viewControllerForItem(_ item: SidebarItem) -> NSViewController {
        if let existing = viewControllers[item] {
            return existing
        }

        let vc: NSViewController
        switch item {
        case .home: vc = PlaceholderViewController.home()
        case .movies: vc = PlaceholderViewController.movies()
        case .tvShows: vc = PlaceholderViewController.tvShows()
        case .other: vc = PlaceholderViewController.other()
        case .favorites: vc = PlaceholderViewController.favorites()
        case .files: vc = PlaceholderViewController.files()
        case .downloads: vc = PlaceholderViewController.downloads()
        case .settings: vc = SettingsViewController()
        }

        viewControllers[item] = vc
        return vc
    }

    // MARK: - Playback

    override func mouseMoved(with event: NSEvent) {
        guard isDirectPlay else { return }
        showControls()
        scheduleHideControls()
    }

    func playFile(path: String) {
        if !isDirectPlay {
            // Switch to player mode within the sidebar layout
            // For now, just log. Full player integration will come later.
            NSLog("Play file: \(path)")
            return
        }
        videoView.loadFile(path: path)
        showControls()
        scheduleHideControls()
    }

    // MARK: - Controls visibility

    private func showControls() {
        controlsView?.animator().alphaValue = 1.0
        NSCursor.unhide()
    }

    private func hideControls() {
        controlsView?.animator().alphaValue = 0.0
        if window?.styleMask.contains(.fullScreen) == true {
            NSCursor.hide()
        }
    }

    private func scheduleHideControls() {
        hideControlsTimer?.invalidate()
        hideControlsTimer = Timer.scheduledTimer(withTimeInterval: 3.0, repeats: false) { [weak self] _ in
            self?.hideControls()
        }
    }

    // MARK: - Actions (menu bar / keyboard)

    @objc func togglePause() {
        videoView?.togglePause()
    }

    @objc func seekForward() {
        videoView?.seek(seconds: 10)
    }

    @objc func seekBackward() {
        videoView?.seek(seconds: -10)
    }

    @objc func toggleFullscreen() {
        window?.toggleFullScreen(nil)
    }

    // MARK: - Key handling

    override func keyDown(with event: NSEvent) {
        guard let chars = event.charactersIgnoringModifiers else {
            super.keyDown(with: event)
            return
        }

        if isDirectPlay {
            switch chars {
            case " ":
                togglePause()
            case "f":
                toggleFullscreen()
            case "m":
                videoView?.toggleMute()
            case "s":
                videoView?.cycleSub()
            case "a":
                videoView?.cycleAudio()
            default:
                switch event.keyCode {
                case 123: seekBackward()
                case 124: seekForward()
                case 126: videoView?.adjustVolume(delta: 5)
                case 125: videoView?.adjustVolume(delta: -5)
                case 53:
                    if window?.styleMask.contains(.fullScreen) == true {
                        toggleFullscreen()
                    }
                default:
                    super.keyDown(with: event)
                }
            }
        } else {
            super.keyDown(with: event)
        }
    }
}
