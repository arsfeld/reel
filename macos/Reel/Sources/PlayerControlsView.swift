import AppKit

/// Translucent overlay with transport controls, seek bar, and time display.
class PlayerControlsView: NSView {
    private weak var videoView: VideoView?
    private var playButton: NSButton!
    private var seekSlider: NSSlider!
    private var timeLabel: NSTextField!
    private var volumeSlider: NSSlider!
    private var fullscreenButton: NSButton!

    init(frame: NSRect, videoView: VideoView) {
        self.videoView = videoView
        super.init(frame: frame)
        setupAppearance()
        setupControls()
        setupObserver()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) not implemented")
    }

    private func setupAppearance() {
        wantsLayer = true
        layer?.backgroundColor = NSColor(white: 0, alpha: 0.7).cgColor
        layer?.cornerRadius = 12
    }

    private func setupControls() {
        // Play/Pause button
        playButton = NSButton(image: NSImage(systemSymbolName: "play.fill", accessibilityDescription: "Play")!,
                              target: self, action: #selector(playPauseClicked))
        playButton.isBordered = false
        playButton.contentTintColor = .white
        addSubview(playButton)

        // Seek slider
        seekSlider = NSSlider(value: 0, minValue: 0, maxValue: 100,
                              target: self, action: #selector(seekChanged))
        seekSlider.isContinuous = true
        addSubview(seekSlider)

        // Time label
        timeLabel = NSTextField(labelWithString: "0:00 / 0:00")
        timeLabel.textColor = .white
        timeLabel.font = .monospacedDigitSystemFont(ofSize: 12, weight: .regular)
        addSubview(timeLabel)

        // Volume slider
        volumeSlider = NSSlider(value: 100, minValue: 0, maxValue: 150,
                                target: self, action: #selector(volumeChanged))
        volumeSlider.isContinuous = true
        addSubview(volumeSlider)

        // Fullscreen button
        fullscreenButton = NSButton(
            image: NSImage(systemSymbolName: "arrow.up.left.and.arrow.down.right", accessibilityDescription: "Fullscreen")!,
            target: self, action: #selector(fullscreenClicked))
        fullscreenButton.isBordered = false
        fullscreenButton.contentTintColor = .white
        addSubview(fullscreenButton)
    }

    override func layout() {
        super.layout()
        let b = bounds
        let padding: CGFloat = 12
        let buttonSize: CGFloat = 30

        // Play button on the left
        playButton.frame = NSRect(x: padding, y: (b.height - buttonSize) / 2,
                                  width: buttonSize, height: buttonSize)

        // Fullscreen button on the right
        fullscreenButton.frame = NSRect(x: b.width - padding - buttonSize,
                                        y: (b.height - buttonSize) / 2,
                                        width: buttonSize, height: buttonSize)

        // Volume slider next to fullscreen
        let volumeWidth: CGFloat = 80
        volumeSlider.frame = NSRect(x: b.width - padding - buttonSize - 8 - volumeWidth,
                                     y: (b.height - 20) / 2,
                                     width: volumeWidth, height: 20)

        // Time label
        let timeLabelWidth: CGFloat = 120
        timeLabel.frame = NSRect(x: volumeSlider.frame.minX - timeLabelWidth - 8,
                                  y: (b.height - 16) / 2,
                                  width: timeLabelWidth, height: 16)

        // Seek slider fills remaining space
        let seekX = playButton.frame.maxX + 8
        let seekWidth = timeLabel.frame.minX - seekX - 8
        seekSlider.frame = NSRect(x: seekX, y: (b.height - 20) / 2,
                                   width: max(seekWidth, 100), height: 20)
    }

    // MARK: - Observer

    private func setupObserver() {
        NotificationCenter.default.addObserver(
            self, selector: #selector(playerStateChanged),
            name: .reelPlayerStateChanged, object: nil
        )
    }

    @objc private func playerStateChanged() {
        guard let video = videoView else { return }

        // Update play/pause icon
        let symbolName = video.paused ? "play.fill" : "pause.fill"
        playButton.image = NSImage(systemSymbolName: symbolName, accessibilityDescription: nil)

        // Update time label
        timeLabel.stringValue = "\(formatTime(video.position)) / \(formatTime(video.duration))"

        // Update seek slider
        if video.duration > 0 {
            seekSlider.doubleValue = (video.position / video.duration) * 100
        }
    }

    // MARK: - Actions

    @objc private func playPauseClicked() {
        videoView?.togglePause()
    }

    @objc private func seekChanged() {
        guard let video = videoView, video.duration > 0 else { return }
        let seconds = (seekSlider.doubleValue / 100) * video.duration
        video.seekAbsolute(seconds: seconds)
    }

    @objc private func volumeChanged() {
        videoView?.setVolume(volumeSlider.doubleValue)
    }

    @objc private func fullscreenClicked() {
        window?.toggleFullScreen(nil)
    }

    // MARK: - Helpers

    private func formatTime(_ seconds: Double) -> String {
        let total = max(0, Int(seconds))
        let h = total / 3600
        let m = (total % 3600) / 60
        let s = total % 60
        if h > 0 {
            return String(format: "%d:%02d:%02d", h, m, s)
        }
        return String(format: "%d:%02d", m, s)
    }
}
