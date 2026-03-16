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
            // Use dlsym to resolve GL proc addresses on macOS
            return dlsym(nil, name) // nil handle = RTLD_DEFAULT
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

            // Controls overlay
            if showControls {
                VStack {
                    Spacer()
                    controlsBar
                        .padding()
                        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 16))
                        .padding()
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
        }
        .onHover { hovering in
            if hovering {
                withAnimation { showControls = true }
                NSCursor.unhide()
                scheduleHide()
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

    private var controlsBar: some View {
        VStack(spacing: 8) {
            // Seek bar
            Slider(value: Binding(
                get: { playerModel.progress },
                set: { newValue in
                    playerModel.seek(to: newValue * playerModel.duration)
                }
            ))

            HStack {
                // Play/Pause
                Button {
                    playerModel.togglePause()
                } label: {
                    Image(systemName: playerModel.isPlaying ? "pause.fill" : "play.fill")
                        .font(.title2)
                }
                .buttonStyle(.plain)

                // Time
                Text("\(playerModel.formattedPosition) / \(playerModel.formattedDuration)")
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(.secondary)

                Spacer()

                // Volume
                HStack(spacing: 4) {
                    Image(systemName: playerModel.volume > 0 ? "speaker.wave.2.fill" : "speaker.slash.fill")
                        .font(.caption)
                    Slider(value: Binding(
                        get: { playerModel.volume },
                        set: { playerModel.setVolume($0) }
                    ), in: 0...150)
                    .frame(width: 80)
                }

                // Fullscreen
                Button {
                    NSApp.keyWindow?.toggleFullScreen(nil)
                } label: {
                    Image(systemName: "arrow.up.left.and.arrow.down.right")
                }
                .buttonStyle(.plain)
            }
        }
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
