import AppKit
import OpenGL.GL3
import ReelCore

/// NSOpenGLView subclass that hosts mpv rendering via libreel.
///
/// Note: Apple deprecated OpenGL (macOS 10.14). For production, migrate to
/// Metal via mpv's --gpu-api=vulkan with MoltenVK, or use CAMetalLayer.
/// OpenGL is used here for initial parity with the Linux GtkGLArea frontend.
class VideoView: NSOpenGLView {
    private var player: OpaquePointer? // ReelPlayer*
    private var renderContext: OpaquePointer? // mpv_render_context*
    private var displayLink: CVDisplayLink?
    private var isPaused = true
    private var currentPosition: Double = 0
    private var currentDuration: Double = 0

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
        let pixelFormat = NSOpenGLPixelFormat(attributes: attrs)!

        super.init(frame: frame, pixelFormat: pixelFormat)!
        wantsBestResolutionOpenGLSurface = true
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) not implemented")
    }

    override func prepareOpenGL() {
        super.prepareOpenGL()

        openGLContext?.makeCurrentContext()

        // Initialize the Reel player via C ABI
        player = reel_player_create()
        guard player != nil else {
            NSLog("Failed to create Reel player")
            return
        }

        // Set up display link for vsync-driven rendering
        setupDisplayLink()

        // Start polling for mpv events
        Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { [weak self] _ in
            self?.pollEvents()
        }
    }

    deinit {
        if let displayLink = displayLink {
            CVDisplayLinkStop(displayLink)
        }
        if let player = player {
            reel_player_destroy(player)
        }
    }

    // MARK: - Rendering

    override func draw(_ dirtyRect: NSRect) {
        guard let player = player, let ctx = openGLContext else { return }
        ctx.makeCurrentContext()

        let bounds = convertToBacking(self.bounds)
        let width = Int32(bounds.width)
        let height = Int32(bounds.height)

        // Get the current FBO (macOS may use a non-zero FBO)
        var fbo: GLint = 0
        glGetIntegerv(GLenum(GL_FRAMEBUFFER_BINDING), &fbo)

        // Render via libreel — this calls mpv_render_context_render internally
        // For now, clear to black. Full mpv render integration requires
        // passing the OpenGL context to libreel's render init.
        glClearColor(0, 0, 0, 1)
        glClear(GLbitfield(GL_COLOR_BUFFER_BIT))

        // TODO: Call libreel render function once C ABI exposes render context
        // reel_player_render(player, fbo, width, height)
        _ = width
        _ = height

        ctx.flushBuffer()
    }

    private func setupDisplayLink() {
        CVDisplayLinkCreateWithActiveCGDisplays(&displayLink)
        guard let displayLink = displayLink else { return }

        let callback: CVDisplayLinkOutputCallback = { _, _, _, _, _, userInfo -> CVReturn in
            let view = Unmanaged<VideoView>.fromOpaque(userInfo!).takeUnretainedValue()
            DispatchQueue.main.async {
                view.needsDisplay = true
            }
            return kCVReturnSuccess
        }

        CVDisplayLinkSetOutputCallback(displayLink, callback, Unmanaged.passUnretained(self).toOpaque())
        CVDisplayLinkStart(displayLink)
    }

    // MARK: - Player controls

    func loadFile(path: String) {
        guard let player = player else { return }
        let result = reel_player_load_file(player, path)
        if result != REEL_OK {
            NSLog("Failed to load file: \(path), error: \(result)")
        }
    }

    func togglePause() {
        guard let player = player else { return }
        reel_player_toggle_pause(player)
    }

    func seek(seconds: Double) {
        guard let player = player else { return }
        reel_player_seek(player, seconds)
    }

    func seekAbsolute(seconds: Double) {
        guard let player = player else { return }
        reel_player_seek_absolute(player, seconds)
    }

    func setVolume(_ volume: Double) {
        guard let player = player else { return }
        reel_player_set_volume(player, volume)
    }

    func adjustVolume(delta: Double) {
        guard let player = player else { return }
        let current = reel_player_get_position(player) // placeholder — need get_volume
        setVolume(min(150, max(0, current + delta)))
    }

    func toggleMute() {
        guard let player = player else { return }
        reel_player_toggle_mute(player)
    }

    func cycleSub() {
        guard let player = player else { return }
        reel_player_cycle_sub(player)
    }

    func cycleAudio() {
        guard let player = player else { return }
        reel_player_cycle_audio(player)
    }

    var position: Double { currentPosition }
    var duration: Double { currentDuration }
    var paused: Bool { isPaused }

    // MARK: - Event polling

    private func pollEvents() {
        guard let player = player else { return }
        currentPosition = reel_player_get_position(player)
        currentDuration = reel_player_get_duration(player)
        let state = reel_player_get_state(player)
        isPaused = (state == REEL_STATE_PAUSED || state == REEL_STATE_IDLE)

        // Notify controls of state changes
        NotificationCenter.default.post(name: .reelPlayerStateChanged, object: self)
    }

    // MARK: - Accept first responder for key events

    override var acceptsFirstResponder: Bool { true }
}

extension Notification.Name {
    static let reelPlayerStateChanged = Notification.Name("ReelPlayerStateChanged")
}
