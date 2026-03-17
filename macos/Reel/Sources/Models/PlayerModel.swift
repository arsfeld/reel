import SwiftUI
import ReelCore

@Observable
final class PlayerModel {
    var isPlaying = false
    var position: Double = 0
    var duration: Double = 0
    var volume: Double = 100
    var state: Int32 = 0 // REEL_STATE_*

    var isActive = false
    var videoWidth: Int64 = 0
    var videoHeight: Int64 = 0
    var currentFilePath: String?
    var currentMediaItemId: Int64 = 0
    var hasError = false
    var errorMessage: String?

    private(set) var player: OpaquePointer?
    private var pollTimer: Timer?
    private var lastSaveTime: Date = .distantPast
    private var renderContextReady = false

    // Weak reference to library for watch progress
    weak var appState: AppState?

    var formattedPosition: String { formatTime(position) }
    var formattedDuration: String { formatTime(duration) }
    var formattedRemaining: String { formatTime(max(0, duration - position)) }

    var progress: Double {
        guard duration > 0 else { return 0 }
        return position / duration
    }

    func createPlayer() {
        guard player == nil else { return }
        player = reel_player_create()
        renderContextReady = false
    }

    func destroyPlayer() {
        stopPolling()
        if let p = player {
            reel_player_destroy(p)
            player = nil
        }
        renderContextReady = false
        isActive = false
        currentFilePath = nil
        currentMediaItemId = 0
        hasError = false
        errorMessage = nil
    }

    /// Start playback. Creates the player and sets isActive to trigger
    /// the SwiftUI view. File loading is deferred until onRenderContextReady().
    func play(filePath: String, mediaItemId: Int64 = 0) {
        // If already playing something, stop it first
        if isActive {
            stop()
        }

        hasError = false
        errorMessage = nil
        createPlayer()
        currentFilePath = filePath
        currentMediaItemId = mediaItemId
        isActive = true  // Triggers PlayerScreen to appear with VideoPlayerView

        // File loading happens in onRenderContextReady() after the OpenGL view
        // calls prepareOpenGL and initializes the mpv render context.
    }

    /// Called by PlayerOpenGLView after mpv render context is successfully initialized.
    func onRenderContextReady() {
        renderContextReady = true
        guard let p = player, let path = currentFilePath else { return }

        let err = reel_player_load_file(p, path)
        guard err == REEL_OK else {
            hasError = true
            errorMessage = "Failed to load: \(path)"
            return
        }

        isPlaying = true
        startPolling()

        // Resume from saved watch progress if available
        resumeFromSavedPosition()
    }

    func togglePause() {
        guard let p = player else { return }
        reel_player_toggle_pause(p)
        // Save progress on pause
        if isPlaying {
            saveProgress()
        }
    }

    func toggleMute() {
        guard let p = player else { return }
        reel_player_toggle_mute(p)
    }

    func seek(to seconds: Double) {
        guard let p = player else { return }
        reel_player_seek_absolute(p, seconds)
    }

    func seekRelative(_ seconds: Double) {
        guard let p = player else { return }
        reel_player_seek(p, seconds)
    }

    func setVolume(_ vol: Double) {
        guard let p = player else { return }
        reel_player_set_volume(p, vol)
        volume = vol
    }

    func stop() {
        guard player != nil else { return }
        saveProgress()
        isPlaying = false
        destroyPlayer()
        NSCursor.unhide()
    }

    func cycleSub() {
        guard let p = player else { return }
        reel_player_cycle_sub(p)
    }

    func cycleAudio() {
        guard let p = player else { return }
        reel_player_cycle_audio(p)
    }

    // MARK: - Watch Progress

    private func resumeFromSavedPosition() {
        guard let lib = appState?.library, currentMediaItemId > 0 else { return }
        var progress = ReelWatchProgressC()
        let err = reel_library_get_watch_progress(lib, currentMediaItemId, &progress)
        if err == REEL_OK && progress.watched == 0 && progress.position_ms > 0 {
            let resumeSeconds = Double(progress.position_ms) / 1000.0
            seek(to: resumeSeconds)
        }
    }

    private func saveProgress() {
        guard let lib = appState?.library, currentMediaItemId > 0 else { return }
        guard duration > 0 else { return }
        let posMs = Int64(position * 1000)
        let durMs = Int64(duration * 1000)
        let watched: Int32 = (position / duration > 0.9) ? 1 : 0
        reel_library_update_watch_progress(lib, currentMediaItemId, posMs, durMs, watched)
        lastSaveTime = Date()
    }

    private func maybeSaveProgress() {
        guard isPlaying, Date().timeIntervalSince(lastSaveTime) >= 10 else { return }
        saveProgress()
    }

    // MARK: - Polling

    private func startPolling() {
        stopPolling()
        pollTimer = Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { [weak self] _ in
            self?.pollState()
        }
    }

    private func stopPolling() {
        pollTimer?.invalidate()
        pollTimer = nil
    }

    private func pollState() {
        guard let p = player else { return }
        reel_player_poll_events(p)
        position = reel_player_get_position(p)
        duration = reel_player_get_duration(p)
        volume = reel_player_get_volume(p)
        state = Int32(reel_player_get_state(p).rawValue)
        isPlaying = state == Int32(REEL_STATE_PLAYING.rawValue)

        // Update video dimensions (available after file loads)
        let w = reel_player_get_video_width(p)
        let h = reel_player_get_video_height(p)
        if w > 0 && h > 0 {
            videoWidth = w
            videoHeight = h
        }

        // Periodically save watch progress
        maybeSaveProgress()

        // Handle end-of-file: playback ended naturally
        if state == Int32(REEL_STATE_STOPPED.rawValue) && isActive && !hasError {
            saveProgress()
            isActive = false
            NSCursor.unhide()
        }
    }

    private func formatTime(_ seconds: Double) -> String {
        guard seconds.isFinite && seconds >= 0 else { return "0:00" }
        let total = Int(seconds)
        let h = total / 3600
        let m = (total % 3600) / 60
        let s = total % 60
        if h > 0 {
            return String(format: "%d:%02d:%02d", h, m, s)
        }
        return String(format: "%d:%02d", m, s)
    }
}
