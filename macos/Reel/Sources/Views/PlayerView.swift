import SwiftUI
import AppKit
import OpenGL.GL3
import ReelCore

/// Bridges the AppKit-based OpenGL video view into SwiftUI.
struct VideoPlayerView: NSViewRepresentable {
    let playerModel: PlayerModel

    func makeNSView(context: Context) -> PlayerOpenGLView {
        let view = PlayerOpenGLView(frame: .zero)
        view.player = playerModel.player
        view.playerModel = playerModel
        return view
    }

    func updateNSView(_ nsView: PlayerOpenGLView, context: Context) {
        // Update player pointer if recreated
        if nsView.player != playerModel.player {
            nsView.player = playerModel.player
        }
    }
}

/// NSOpenGLView subclass that renders mpv video frames via libreel's C ABI.
///
/// Uses CVDisplayLink for vsync-driven rendering. The mpv render context
/// is initialized in prepareOpenGL, then PlayerModel is notified to load the file.
class PlayerOpenGLView: NSOpenGLView {
    var player: OpaquePointer?  // ReelPlayer*
    weak var playerModel: PlayerModel?
    private var displayLink: CVDisplayLink?
    private var renderReady = false
    private var retainedSelf: Unmanaged<PlayerOpenGLView>?

    override init(frame: NSRect) {
        // Request OpenGL 3.3 Core Profile
        let attrs: [NSOpenGLPixelFormatAttribute] = [
            UInt32(NSOpenGLPFAOpenGLProfile), UInt32(NSOpenGLProfileVersion3_2Core),
            UInt32(NSOpenGLPFAColorSize), 24,
            UInt32(NSOpenGLPFAAlphaSize), 8,
            UInt32(NSOpenGLPFADepthSize), 24,
            UInt32(NSOpenGLPFADoubleBuffer),
            UInt32(NSOpenGLPFAAccelerated),
            0
        ]
        let pf = NSOpenGLPixelFormat(attributes: attrs)!
        super.init(frame: frame, pixelFormat: pf)!
        wantsBestResolutionOpenGLSurface = true
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) not implemented")
    }

    override func prepareOpenGL() {
        super.prepareOpenGL()
        openGLContext?.makeCurrentContext()

        // Vsync
        var swapInterval: GLint = 1
        openGLContext?.setValues(&swapInterval, for: .swapInterval)

        initializeRenderContext()
        setupUpdateCallback()
        setupDisplayLink()

        // Notify PlayerModel that the render context is ready — it can now load the file
        DispatchQueue.main.async { [weak self] in
            self?.playerModel?.onRenderContextReady()
        }
    }

    deinit {
        stopDisplayLink()
    }

    // MARK: - Render Context

    private func initializeRenderContext() {
        guard let player = player else { return }
        let result = reel_player_init_render(player) { _, name -> UnsafeMutableRawPointer? in
            guard let name = name else { return nil }
            // Load GL functions from the OpenGL framework (dlsym(nil) doesn't find them on macOS)
            let handle = dlopen("/System/Library/Frameworks/OpenGL.framework/OpenGL", RTLD_LAZY)
            if let handle, let sym = dlsym(handle, name) { return sym }
            return dlsym(nil, name)
        }
        renderReady = (result == REEL_OK)
        if !renderReady {
            print("Failed to initialize mpv render context: \(result)")
        }
    }

    private func setupUpdateCallback() {
        guard let player = player, renderReady else { return }
        // Prevent deallocation while mpv holds the callback reference
        retainedSelf = Unmanaged.passRetained(self)
        let viewPtr = retainedSelf!.toOpaque()
        reel_player_set_render_update_callback(player, { ctx in
            // Fires on mpv's internal thread — do NOT call any mpv API here
            guard let ctx = ctx else { return }
            let view = Unmanaged<PlayerOpenGLView>.fromOpaque(ctx).takeUnretainedValue()
            DispatchQueue.main.async {
                view.needsDisplay = true
            }
        }, viewPtr)
    }

    // MARK: - Rendering

    override func draw(_ dirtyRect: NSRect) {
        guard let player = player, renderReady,
              let ctx = openGLContext else {
            // Render black if not ready
            super.draw(dirtyRect)
            return
        }
        ctx.makeCurrentContext()

        // Use backing pixel dimensions for Retina
        let bounds = convertToBacking(self.bounds)
        var fbo: GLint = 0
        glGetIntegerv(GLenum(GL_FRAMEBUFFER_BINDING), &fbo)

        reel_player_render(player, fbo, Int32(bounds.width), Int32(bounds.height))
        ctx.flushBuffer()
    }

    // MARK: - CVDisplayLink

    private func setupDisplayLink() {
        CVDisplayLinkCreateWithActiveCGDisplays(&displayLink)
        guard let displayLink = displayLink else { return }

        let callback: CVDisplayLinkOutputCallback = { _, _, _, _, _, userInfo -> CVReturn in
            guard let userInfo = userInfo else { return kCVReturnSuccess }
            let view = Unmanaged<PlayerOpenGLView>.fromOpaque(userInfo).takeUnretainedValue()
            DispatchQueue.main.async {
                view.needsDisplay = true
            }
            return kCVReturnSuccess
        }

        CVDisplayLinkSetOutputCallback(displayLink, callback, Unmanaged.passUnretained(self).toOpaque())
        CVDisplayLinkStart(displayLink)
    }

    private func stopDisplayLink() {
        if let displayLink = displayLink {
            CVDisplayLinkStop(displayLink)
            self.displayLink = nil
        }
        // Release the retained self from the update callback
        if let retained = retainedSelf {
            // Clear the callback first so mpv doesn't call into freed memory
            if let player = player {
                reel_player_set_render_update_callback(player, nil, nil)
            }
            retained.release()
            retainedSelf = nil
        }
    }

    override func viewWillMove(toWindow newWindow: NSWindow?) {
        super.viewWillMove(toWindow: newWindow)
        if newWindow == nil {
            stopDisplayLink()
        }
    }

    // MARK: - Keyboard

    override var acceptsFirstResponder: Bool { true }

    override func keyDown(with event: NSEvent) {
        guard let model = playerModel else {
            super.keyDown(with: event)
            return
        }

        switch event.keyCode {
        case 49: // Space
            model.togglePause()
        case 123: // Left arrow
            model.seekRelative(-10)
        case 124: // Right arrow
            model.seekRelative(10)
        case 126: // Up arrow
            model.setVolume(min(150, model.volume + 5))
        case 125: // Down arrow
            model.setVolume(max(0, model.volume - 5))
        case 3: // F
            window?.toggleFullScreen(nil)
        case 46: // M
            model.toggleMute()
        case 1: // S
            model.cycleSub()
        case 0: // A
            model.cycleAudio()
        case 53: // Escape — handled by parent view
            break
        default:
            super.keyDown(with: event)
        }
    }
}

/// Full player screen with controls overlay.
struct PlayerScreen: View {
    @Environment(AppState.self) private var appState
    @Bindable var playerModel: PlayerModel
    @Binding var columnVisibility: NavigationSplitViewVisibility
    @State private var showControls = true
    @State private var hideTimer: Timer?

    var body: some View {
        ZStack {
            // Video view
            VideoPlayerView(playerModel: playerModel)
                .ignoresSafeArea()

            // Loading spinner while file is loading but not yet playing
            if playerModel.isActive && !playerModel.isPlaying && !playerModel.hasError {
                ProgressView()
                    .scaleEffect(2)
                    .tint(.white)
            }

            // Error overlay
            if playerModel.hasError {
                VStack(spacing: 12) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .font(.largeTitle)
                        .foregroundStyle(.yellow)
                    Text("Playback Error")
                        .font(.headline)
                    Text(playerModel.errorMessage ?? "Failed to load file")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            // Infuse desktop-style OSD
            if showControls {
                VStack {
                    Spacer()
                    controlsPanel
                        .frame(maxWidth: 700)
                        .padding(.bottom, 36)
                }
                .transition(.opacity)
            }
        }
        .background(.black)
        .onAppear {
            columnVisibility = .detailOnly
            scheduleHide()
        }
        .onDisappear {
            columnVisibility = .all
            hideTimer?.invalidate()
            // Clear aspect ratio lock so the window is freely resizable again
            if let window = NSApp.keyWindow {
                window.resizeIncrements = NSSize(width: 1, height: 1)
            }
        }
        .onChange(of: playerModel.videoWidth) {
            resizeWindowForVideo()
        }
        .onContinuousHover { phase in
            switch phase {
            case .active:
                withAnimation { showControls = true }
                NSCursor.unhide()
                scheduleHide()
            case .ended:
                break
            }
        }
        .onTapGesture {
            withAnimation { showControls.toggle() }
            if showControls {
                NSCursor.unhide()
                scheduleHide()
            }
        }
    }

    // MARK: - Infuse Desktop OSD Panel

    private var controlsPanel: some View {
        VStack(spacing: 0) {
            // Top row: volume slider + transport + utility buttons
            HStack(spacing: 0) {
                // Volume (left)
                HStack(spacing: 8) {
                    Slider(value: Binding(
                        get: { playerModel.volume / 100 },
                        set: { playerModel.setVolume($0 * 100) }
                    ))
                    .tint(.white.opacity(0.6))
                    .frame(width: 90)
                }
                .frame(width: 110, alignment: .leading)

                Spacer()

                // Transport (center)
                HStack(spacing: 24) {
                    osdButton("backward.end.fill", size: 16) { playerModel.seekRelative(-10) }
                    osdButton("gobackward.10", size: 20) { playerModel.seekRelative(-10) }

                    Button { playerModel.togglePause() } label: {
                        Image(systemName: playerModel.isPlaying ? "pause.fill" : "play.fill")
                            .font(.system(size: 28, weight: .medium))
                            .foregroundStyle(.white)
                            .frame(width: 40, height: 40)
                    }
                    .buttonStyle(.plain)

                    osdButton("goforward.30", size: 20) { playerModel.seekRelative(30) }
                    osdButton("forward.end.fill", size: 16) { playerModel.seekRelative(30) }
                }

                Spacer()

                // Utility buttons (right)
                HStack(spacing: 14) {
                    osdButton("gearshape", size: 15) { }
                    osdButton("captions.bubble", size: 15) { playerModel.cycleSub() }
                    osdButton("pip", size: 15) { }
                    osdButton("arrow.up.left.and.arrow.down.right", size: 14) {
                        NSApp.keyWindow?.toggleFullScreen(nil)
                    }
                    osdButton("xmark", size: 14) { playerModel.stop() }
                }
            }
            .padding(.horizontal, 16)
            .padding(.top, 12)
            .padding(.bottom, 8)

            // Bottom row: progress bar with timestamps
            HStack(spacing: 8) {
                Text(playerModel.formattedPosition)
                    .font(.system(size: 11, weight: .medium).monospacedDigit())
                    .foregroundStyle(.white.opacity(0.6))
                    .frame(width: 50, alignment: .trailing)

                Slider(value: Binding(
                    get: { playerModel.progress },
                    set: { playerModel.seek(to: $0 * playerModel.duration) }
                ))
                .tint(.white.opacity(0.5))

                Text("-\(playerModel.formattedRemaining)")
                    .font(.system(size: 11, weight: .medium).monospacedDigit())
                    .foregroundStyle(.white.opacity(0.6))
                    .frame(width: 50, alignment: .leading)
            }
            .padding(.horizontal, 14)
            .padding(.bottom, 10)
        }
        .background(
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .fill(.black.opacity(0.85))
                .overlay(
                    RoundedRectangle(cornerRadius: 14, style: .continuous)
                        .strokeBorder(.white.opacity(0.1), lineWidth: 0.5)
                )
        )
    }

    private func osdButton(_ icon: String, size: CGFloat, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: icon)
                .font(.system(size: size, weight: .medium))
                .foregroundStyle(.white.opacity(0.85))
                .frame(width: 30, height: 30)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private var volumeIcon: String {
        if playerModel.volume <= 0 { return "speaker.slash.fill" }
        if playerModel.volume < 34 { return "speaker.wave.1.fill" }
        if playerModel.volume < 67 { return "speaker.wave.2.fill" }
        return "speaker.wave.3.fill"
    }

    private func resizeWindowForVideo() {
        guard let window = NSApp.keyWindow,
              !window.styleMask.contains(.fullScreen),
              playerModel.videoWidth > 0, playerModel.videoHeight > 0 else { return }

        let aspect = CGFloat(playerModel.videoWidth) / CGFloat(playerModel.videoHeight)
        guard let screen = window.screen else { return }

        let maxSize = screen.visibleFrame.size
        var newWidth = min(CGFloat(playerModel.videoWidth), maxSize.width * 0.85)
        var newHeight = newWidth / aspect
        if newHeight > maxSize.height * 0.85 {
            newHeight = maxSize.height * 0.85
            newWidth = newHeight * aspect
        }

        let frame = window.frame
        let centerX = frame.midX - newWidth / 2
        let centerY = frame.midY - newHeight / 2
        let newFrame = NSRect(x: centerX, y: centerY, width: newWidth, height: newHeight)
        window.setFrame(newFrame, display: true, animate: true)
        window.aspectRatio = NSSize(width: CGFloat(playerModel.videoWidth), height: CGFloat(playerModel.videoHeight))
    }

    private func scheduleHide() {
        hideTimer?.invalidate()
        hideTimer = Timer.scheduledTimer(withTimeInterval: 3, repeats: false) { _ in
            if playerModel.isPlaying {
                withAnimation { showControls = false }
                // Hide cursor when controls fade out during fullscreen
                if let window = NSApp.keyWindow, window.styleMask.contains(.fullScreen) {
                    NSCursor.hide()
                }
            }
        }
    }
}
