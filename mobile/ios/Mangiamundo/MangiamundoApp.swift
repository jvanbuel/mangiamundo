import SwiftUI

@main
struct MangiamundoApp: App {
    /// For the push device token, which only arrives through an app delegate.
    @UIApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @State private var model = AppModel()
    @State private var loaderShown = true

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(model)
                .tint(Theme.accent)
                .overlay {
                    if loaderShown {
                        StartupLoaderView(ready: model.ready) { loaderShown = false }
                    }
                }
                .task { await model.start() }
        }
    }
}
