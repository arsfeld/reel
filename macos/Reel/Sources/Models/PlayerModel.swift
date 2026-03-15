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
    var currentFilePath: String?

    private var player: OpaquePointer?
    private var pollTimer: Timer?

    var formattedPosition: String { formatTime(position) }
    var formattedDuration: String { formatTime(duration) }

    var progress: Double {
        guard duration > 0 else { return 0 }
        return position / duration
    }

    func createPlayer() {
        guard player == nil else { return }
        player = reel_player_create()
    }

    func destroyPlayer() {
        stopPolling()
        if let p = player {
            reel_player_destroy(p)
            player = nil
        }
        isActive = false
        currentFilePath = nil
    }

    func play(filePath: String) {
        createPlayer()
        guard let p = player else { return }
        let err = reel_player_load_file(p, filePath)
        guard err.rawValue == 0 else { return }
        currentFilePath = filePath
        isPlaying = true
        isActive = true
        startPolling()
    }

    func togglePause() {
        guard let p = player else { return }
        reel_player_toggle_pause(p)
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
        guard let p = player else { return }
        reel_player_stop(p)
        isPlaying = false
        isActive = false
        stopPolling()
    }

    func cycleSub() {
        guard let p = player else { return }
        reel_player_cycle_sub(p)
    }

    func cycleAudio() {
        guard let p = player else { return }
        reel_player_cycle_audio(p)
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

        if state == Int32(REEL_STATE_STOPPED.rawValue) && isActive {
            // Playback ended
            isActive = false
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
