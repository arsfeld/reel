import SwiftUI

@main
struct ReelApp: App {
    @StateObject private var appModel = AppModel()
    
    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(appModel)
                .frame(minWidth: 800, minHeight: 600)
        }
        .windowStyle(.titleBar)
        .windowToolbarStyle(.unified)
        .commands {
            CommandGroup(replacing: .appInfo) {
                Button("About Reel") {
                    appModel.showAbout()
                }
            }
        }
        
        Settings {
            SettingsView()
                .environmentObject(appModel)
        }
    }
}