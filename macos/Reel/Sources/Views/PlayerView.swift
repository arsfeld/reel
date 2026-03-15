import SwiftUI
import AppKit
import OpenGL.GL3
import ReelCore

/// Bridges the AppKit-based OpenGL video view into SwiftUI.
struct VideoPlayerView: NSViewRepresentable {
    let playerModel: PlayerModel

    func makeNSView(context: Context) -> NSView {
        let view = PlayerNSView()
        view.playerModel = playerModel
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        // State is managed by PlayerModel polling
    }
}

/// Minimal NSView that renders black and will host mpv rendering.
/// Full Metal/Vulkan integration is Phase 5b — for now this provides
/// the view container and keyboard event handling.
class PlayerNSView: NSView {
    var playerModel: PlayerModel?

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        layer?.backgroundColor = NSColor.black.cgColor
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) not implemented")
    }

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
            toggleFullscreen()
        case 46: // M
            if let p = model as? PlayerModel {
                // Toggle mute via C ABI
            }
        case 53: // Escape — handled by parent view
            break
        default:
            super.keyDown(with: event)
        }
    }

    private func toggleFullscreen() {
        window?.toggleFullScreen(nil)
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
                scheduleHide()
            }
        }
        .onTapGesture {
            withAnimation { showControls.toggle() }
            if showControls { scheduleHide() }
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
            }
        }
    }
}
