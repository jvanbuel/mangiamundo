import GoogleSignIn
import SwiftUI

@main
struct MangiamundoApp: App {
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
                // Google's sign-in sheet returns through the app's URL scheme.
                .onOpenURL { GIDSignIn.sharedInstance.handle($0) }
        }
    }
}
