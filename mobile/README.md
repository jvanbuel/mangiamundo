# Mangiamundo mobile apps

Two fully native apps that talk to the same backend API as the web app:

- `android/` — Kotlin and Jetpack Compose.
- `ios/` — Swift and SwiftUI (iOS 17+).
- `shared/world.json` — country outlines both apps draw. Regenerate with
  `node tools/build-world.mjs` from the repository root.

Neither app uses a map SDK. The globe (orthographic projection) and the flat map
(Web Mercator) are drawn straight from the country outlines on a native canvas, so
there are no API keys or tiles to manage and both platforms look the same as the web
app. Light and dark mode follow the system setting.

## Android

Open `android/` in Android Studio, or from the command line:

```sh
cd android
./gradlew :core:test            # map maths + API models; needs no Android SDK
./gradlew :app:assembleDebug    # needs the Android SDK (ANDROID_HOME or local.properties)
```

The app reads the backend address from `mangiamundo.apiBase` in `gradle.properties`.
The default `http://10.0.2.2:8080` reaches a backend running on your machine from the
emulator. For a real phone, point it at a reachable address; release builds need HTTPS.

## iOS

The Xcode project is generated with [XcodeGen](https://github.com/yonaskolb/XcodeGen):

```sh
brew install xcodegen
cd ios
xcodegen
open Mangiamundo.xcodeproj
```

The backend address is the `APIBaseURL` entry in `project.yml` (default
`http://localhost:8080`, which works from the simulator). Unit tests run with ⌘U.
