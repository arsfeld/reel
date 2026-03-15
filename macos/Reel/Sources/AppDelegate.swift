import AppKit
import ReelCore

class AppDelegate: NSObject, NSApplicationDelegate {
    var mainWindow: MainWindow?

    func applicationDidFinishLaunching(_ notification: Notification) {
        setupMenuBar()
        mainWindow = MainWindow()
        mainWindow?.showWindow(nil)

        // If launched with a file argument, play it
        let args = ProcessInfo.processInfo.arguments
        if args.count > 1 {
            mainWindow?.playFile(path: args[1])
        }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        return true
    }

    func application(_ sender: NSApplication, openFile filename: String) -> Bool {
        mainWindow?.playFile(path: filename)
        return true
    }

    private func setupMenuBar() {
        let mainMenu = NSMenu()

        // App menu
        let appMenu = NSMenu()
        appMenu.addItem(withTitle: "About Reel", action: #selector(NSApplication.orderFrontStandardAboutPanel(_:)), keyEquivalent: "")
        appMenu.addItem(.separator())
        appMenu.addItem(withTitle: "Quit Reel", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")

        let appMenuItem = NSMenuItem()
        appMenuItem.submenu = appMenu
        mainMenu.addItem(appMenuItem)

        // File menu
        let fileMenu = NSMenu(title: "File")
        fileMenu.addItem(withTitle: "Open...", action: #selector(openFile(_:)), keyEquivalent: "o")
        fileMenu.addItem(.separator())
        fileMenu.addItem(withTitle: "Close Window", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")

        let fileMenuItem = NSMenuItem()
        fileMenuItem.submenu = fileMenu
        mainMenu.addItem(fileMenuItem)

        // Playback menu
        let playbackMenu = NSMenu(title: "Playback")
        playbackMenu.addItem(withTitle: "Play/Pause", action: #selector(MainWindow.togglePause), keyEquivalent: " ")
        playbackMenu.addItem(withTitle: "Skip Forward", action: #selector(MainWindow.seekForward), keyEquivalent: String(Character(UnicodeScalar(NSRightArrowFunctionKey)!)))
        playbackMenu.addItem(withTitle: "Skip Backward", action: #selector(MainWindow.seekBackward), keyEquivalent: String(Character(UnicodeScalar(NSLeftArrowFunctionKey)!)))
        playbackMenu.addItem(.separator())
        playbackMenu.addItem(withTitle: "Toggle Fullscreen", action: #selector(MainWindow.toggleFullscreen), keyEquivalent: "f")

        let playbackMenuItem = NSMenuItem()
        playbackMenuItem.submenu = playbackMenu
        mainMenu.addItem(playbackMenuItem)

        // View menu
        let viewMenu = NSMenu(title: "View")
        viewMenu.addItem(withTitle: "Toggle Sidebar", action: #selector(NSSplitViewController.toggleSidebar(_:)), keyEquivalent: "s")
        viewMenu.items.last?.keyEquivalentModifierMask = [.command, .control]

        let viewMenuItem = NSMenuItem()
        viewMenuItem.submenu = viewMenu
        mainMenu.addItem(viewMenuItem)

        NSApplication.shared.mainMenu = mainMenu
    }

    @objc func openFile(_ sender: Any?) {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.movie, .mpeg4Movie, .quickTimeMovie, .avi]
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false

        if panel.runModal() == .OK, let url = panel.url {
            mainWindow?.playFile(path: url.path)
        }
    }
}
