import AppKit
import ReelCore

class MainWindow: NSWindowController {
    private var videoView: VideoView!
    private var controlsView: PlayerControlsView!
    private var hideControlsTimer: Timer?

    convenience init() {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1280, height: 720),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = "Reel"
        window.center()
        window.isReleasedWhenClosed = false
        window.minSize = NSSize(width: 640, height: 360)
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.backgroundColor = .black

        self.init(window: window)
        setupViews()
        setupTrackingArea()
    }

    private func setupViews() {
        guard let contentView = window?.contentView else { return }

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

    override func mouseMoved(with event: NSEvent) {
        showControls()
        scheduleHideControls()
    }

    func playFile(path: String) {
        videoView.loadFile(path: path)
        showControls()
        scheduleHideControls()
    }

    // MARK: - Controls visibility

    private func showControls() {
        controlsView.animator().alphaValue = 1.0
        NSCursor.unhide()
    }

    private func hideControls() {
        controlsView.animator().alphaValue = 0.0
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
        videoView.togglePause()
    }

    @objc func seekForward() {
        videoView.seek(seconds: 10)
    }

    @objc func seekBackward() {
        videoView.seek(seconds: -10)
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

        switch chars {
        case " ":
            togglePause()
        case "f":
            toggleFullscreen()
        case "m":
            videoView.toggleMute()
        case "s":
            videoView.cycleSub()
        case "a":
            videoView.cycleAudio()
        default:
            switch event.keyCode {
            case 123: // Left arrow
                seekBackward()
            case 124: // Right arrow
                seekForward()
            case 126: // Up arrow
                videoView.adjustVolume(delta: 5)
            case 125: // Down arrow
                videoView.adjustVolume(delta: -5)
            case 53: // Escape
                if window?.styleMask.contains(.fullScreen) == true {
                    toggleFullscreen()
                }
            default:
                super.keyDown(with: event)
            }
        }
    }
}
