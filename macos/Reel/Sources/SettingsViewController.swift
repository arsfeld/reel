import AppKit
import ReelCore

class SettingsViewController: NSViewController {
    private enum PlexState {
        case disconnected
        case waitingForAuth
        case connected(serverName: String, serverId: String)
    }

    private var plexState: PlexState = .disconnected
    private var plexWrapper: OpaquePointer?
    private var pollTimer: Timer?
    private var contentStack: NSStackView!

    override func loadView() {
        let scrollView = NSScrollView()
        scrollView.hasVerticalScroller = true
        scrollView.autohidesScrollers = true
        scrollView.drawsBackground = false

        contentStack = NSStackView()
        contentStack.orientation = .vertical
        contentStack.alignment = .leading
        contentStack.spacing = 24
        contentStack.translatesAutoresizingMaskIntoConstraints = false
        contentStack.edgeInsets = NSEdgeInsets(top: 24, left: 24, bottom: 24, right: 24)

        let clipView = NSClipView()
        clipView.documentView = contentStack
        clipView.drawsBackground = false
        scrollView.contentView = clipView

        NSLayoutConstraint.activate([
            contentStack.leadingAnchor.constraint(equalTo: clipView.leadingAnchor),
            contentStack.trailingAnchor.constraint(equalTo: clipView.trailingAnchor),
            contentStack.topAnchor.constraint(equalTo: clipView.topAnchor),
        ])

        self.view = scrollView
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        loadPlexState()
        rebuildUI()
    }

    override func viewWillDisappear() {
        super.viewWillDisappear()
        stopPolling()
    }

    // MARK: - State Management

    private func loadPlexState() {
        let app = AppDelegate.shared
        guard let lib = app.library else { return }

        var count: Int32 = 0
        let rc = reel_server_list(lib, nil, &count)
        if rc == REEL_OK, count > 0 {
            var servers = [ReelServerC](repeating: ReelServerC(id: nil, name: nil, connection_uri: nil), count: Int(count))
            let rc2 = reel_server_list(lib, &servers, &count)
            if rc2 == REEL_OK, count > 0, let namePtr = servers[0].name, let idPtr = servers[0].id {
                let name = String(cString: namePtr)
                let id = String(cString: idPtr)
                plexState = .connected(serverName: name, serverId: id)
                return
            }
        }
        plexState = .disconnected
    }

    // MARK: - UI Construction

    private func rebuildUI() {
        for view in contentStack.arrangedSubviews {
            contentStack.removeArrangedSubview(view)
            view.removeFromSuperview()
        }

        let title = NSTextField(labelWithString: "Settings")
        title.font = .systemFont(ofSize: 24, weight: .bold)
        contentStack.addArrangedSubview(title)

        contentStack.addArrangedSubview(makeSeparator())
        contentStack.addArrangedSubview(buildPlexSection())
    }

    private func buildPlexSection() -> NSView {
        let section = NSStackView()
        section.orientation = .vertical
        section.alignment = .leading
        section.spacing = 12

        let header = NSTextField(labelWithString: "Plex Media Server")
        header.font = .systemFont(ofSize: 16, weight: .semibold)
        section.addArrangedSubview(header)

        switch plexState {
        case .disconnected:
            let desc = NSTextField(wrappingLabelWithString: "Connect to your Plex account to stream your media library.")
            desc.font = .systemFont(ofSize: 13)
            desc.textColor = .secondaryLabelColor
            desc.preferredMaxLayoutWidth = 400
            section.addArrangedSubview(desc)

            let button = NSButton(title: "Connect to Plex", target: self, action: #selector(connectToPlex))
            button.bezelStyle = .rounded
            button.controlSize = .large
            section.addArrangedSubview(button)

        case .waitingForAuth:
            let spinner = NSProgressIndicator()
            spinner.style = .spinning
            spinner.controlSize = .small
            spinner.startAnimation(nil)

            let waitLabel = NSTextField(labelWithString: "Waiting for Plex authorization...")
            waitLabel.font = .systemFont(ofSize: 13)
            waitLabel.textColor = .secondaryLabelColor

            let row = NSStackView(views: [spinner, waitLabel])
            row.orientation = .horizontal
            row.spacing = 8
            section.addArrangedSubview(row)

            let cancelButton = NSButton(title: "Cancel", target: self, action: #selector(cancelPlex))
            cancelButton.bezelStyle = .rounded
            section.addArrangedSubview(cancelButton)

        case .connected(let serverName, _):
            let row = NSStackView()
            row.orientation = .horizontal
            row.spacing = 8

            let serverIcon = NSImageView()
            if #available(macOS 11.0, *) {
                serverIcon.image = NSImage(systemSymbolName: "server.rack", accessibilityDescription: "Server")
            }
            serverIcon.translatesAutoresizingMaskIntoConstraints = false
            NSLayoutConstraint.activate([
                serverIcon.widthAnchor.constraint(equalToConstant: 20),
                serverIcon.heightAnchor.constraint(equalToConstant: 20),
            ])
            row.addArrangedSubview(serverIcon)

            let nameLabel = NSTextField(labelWithString: serverName)
            nameLabel.font = .systemFont(ofSize: 14, weight: .medium)
            row.addArrangedSubview(nameLabel)

            section.addArrangedSubview(row)

            let disconnectButton = NSButton(title: "Disconnect", target: self, action: #selector(disconnectPlex))
            disconnectButton.bezelStyle = .rounded
            section.addArrangedSubview(disconnectButton)
        }

        return section
    }

    private func makeSeparator() -> NSView {
        let separator = NSBox()
        separator.boxType = .separator
        separator.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            separator.widthAnchor.constraint(greaterThanOrEqualToConstant: 400),
        ])
        return separator
    }

    // MARK: - Actions

    @objc private func connectToPlex() {
        let app = AppDelegate.shared
        guard let db = app.db else { return }

        // Show waiting state immediately
        plexState = .waitingForAuth
        rebuildUI()

        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self = self else { return }

            let plex = reel_plex_create(db)
            guard let plex = plex else {
                DispatchQueue.main.async {
                    self.plexState = .disconnected
                    self.rebuildUI()
                }
                return
            }
            self.plexWrapper = plex

            // Request a PIN (needed to get the code for the auth URL)
            guard reel_plex_request_pin(plex) != nil else {
                NSLog("Plex: PIN request failed")
                reel_plex_destroy(plex)
                self.plexWrapper = nil
                DispatchQueue.main.async {
                    self.plexState = .disconnected
                    self.rebuildUI()
                }
                return
            }

            // Get the full auth URL and open the browser
            guard let urlPtr = reel_plex_get_auth_url(plex) else {
                NSLog("Plex: failed to get auth URL")
                reel_plex_destroy(plex)
                self.plexWrapper = nil
                DispatchQueue.main.async {
                    self.plexState = .disconnected
                    self.rebuildUI()
                }
                return
            }
            let authUrl = String(cString: urlPtr)

            DispatchQueue.main.async {
                if let url = URL(string: authUrl) {
                    NSWorkspace.shared.open(url)
                }
                self.startPolling()
            }
        }
    }

    @objc private func cancelPlex() {
        stopPolling()
        if let plex = plexWrapper {
            reel_plex_destroy(plex)
            plexWrapper = nil
        }
        plexState = .disconnected
        rebuildUI()
    }

    @objc private func disconnectPlex() {
        let app = AppDelegate.shared
        guard let lib = app.library else { return }

        if case .connected(_, let serverId) = plexState {
            reel_server_delete(lib, serverId)
        }

        plexState = .disconnected
        rebuildUI()
    }

    // MARK: - Polling

    private func startPolling() {
        NSLog("Plex: starting poll timer")
        pollTimer = Timer.scheduledTimer(withTimeInterval: 2.0, repeats: true) { [weak self] _ in
            NSLog("Plex: poll tick")
            self?.pollForAuth()
        }
    }

    private func stopPolling() {
        pollTimer?.invalidate()
        pollTimer = nil
    }

    private func pollForAuth() {
        guard let plex = plexWrapper else {
            stopPolling()
            return
        }

        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self = self else { return }

            guard let tokenPtr = reel_plex_poll_pin(plex) else {
                return
            }
            let token = String(cString: tokenPtr)
            NSLog("Plex: authenticated with token length %d", token.count)

            NSLog("Plex: discovering servers...")
            var serverBuf = [ReelPlexServerC](
                repeating: ReelPlexServerC(id: nil, name: nil, uri: nil, access_token: nil),
                count: 16
            )
            let count = reel_plex_discover_servers_with_lib(plex, &serverBuf, 16, AppDelegate.shared.library)
            NSLog("Plex: discovered %d servers", count)

            DispatchQueue.main.async {
                self.stopPolling()

                if count > 0,
                   let idPtr = serverBuf[0].id,
                   let namePtr = serverBuf[0].name,
                   let uriPtr = serverBuf[0].uri,
                   let tokenPtr = serverBuf[0].access_token {
                    let serverId = String(cString: idPtr)
                    let serverName = String(cString: namePtr)
                    let serverUri = String(cString: uriPtr)
                    let serverToken = String(cString: tokenPtr)

                    if let lib = AppDelegate.shared.library {
                        reel_server_upsert(lib, serverId, serverName, serverUri,
                                           serverToken.isEmpty ? nil : serverToken)
                    }

                    self.plexState = .connected(serverName: serverName, serverId: serverId)
                } else {
                    self.plexState = .disconnected
                }

                reel_plex_destroy(plex)
                self.plexWrapper = nil
                self.rebuildUI()
            }
        }
    }
}
