import SwiftUI

@main
struct MangiamundoApp: App {
    @State private var model = AppModel()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(model)
                .tint(Theme.accent)
                .task { await model.start() }
        }
    }
}
