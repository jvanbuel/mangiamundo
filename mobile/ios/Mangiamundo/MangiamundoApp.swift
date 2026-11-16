import SwiftUI

@main
struct MangiamundoApp: App {
    @State private var model = AppModel()
    @State private var loading = true

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(model)
                .tint(Theme.accent)
                .task { await model.start() }
                .overlay {
                    if loading {
                        Loader(ready: model.ready) {
                            withAnimation(.easeOut(duration: 0.25)) { loading = false }
                        }
                        .transition(.opacity)
                    }
                }
        }
    }
}
