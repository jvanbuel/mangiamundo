# Releasing the mobile apps

Neither app is in a store yet. This page says how a release build is made, which
secrets the two release workflows need, and what is still to do by hand for each store
(issues #68 for Google Play and #69 for the App Store).

Both apps talk to `https://mangiamundo.com` in a release build, and to a backend on the
developer's machine otherwise. Both show their version, build number and commit at the
bottom of Settings.

## Android

### A release build

```sh
cd mobile/android
./gradlew :app:bundleRelease
```

writes `app/build/outputs/bundle/release/app-release.aab`. Without a keystore the bundle
is not signed; Play does not take it, but the build works, which is what a pull request
needs.

The build reads these from the environment, or else from a Gradle property (for
instance in `~/.gradle/gradle.properties`). None of them belongs in the repository.

| Environment variable | Gradle property | What |
| --- | --- | --- |
| `MANGIAMUNDO_KEYSTORE_FILE` | `mangiamundo.keystoreFile` | Path of the upload keystore; relative paths start at `mobile/android` |
| `MANGIAMUNDO_KEYSTORE_PASSWORD` | `mangiamundo.keystorePassword` | Its password |
| `MANGIAMUNDO_KEY_ALIAS` | `mangiamundo.keyAlias` | The alias of the upload key in it |
| `MANGIAMUNDO_KEY_PASSWORD` | `mangiamundo.keyPassword` | That key's password |
| `MANGIAMUNDO_VERSION_CODE` | `mangiamundo.versionCode` | The version code; 1 when not given |

The version name is `versionName` in `mobile/android/app/build.gradle.kts`.

The release build is not shrunk with R8 (`isMinifyEnabled = false`). The bundle is about
29 MB. Turning R8 on makes it smaller, but nothing in the repository runs a shrunk build,
so a broken one would only show on a phone.

### The upload key

Made once, and kept somewhere safe outside the repository:

```sh
keytool -genkeypair -v -keystore upload.jks -alias upload -keyalg RSA -keysize 2048 -validity 10000
```

With Play App Signing, Google holds the key the app is signed with for users; this one
only proves an upload is ours, and Google can replace it if it is lost.

### From GitHub

Actions → **Android release** → Run workflow (`.github/workflows/android-release.yml`).
It runs the core tests, builds the bundle signed with the upload key, checks that it is
signed, and keeps it as an artifact of the run. The version code is the run's number.
Nothing is sent to Play: download the artifact and upload it in the Play Console.

Repository secrets (Settings → Secrets and variables → Actions). The workflow stops
with a message naming the ones that are missing.

| Secret | What |
| --- | --- |
| `ANDROID_KEYSTORE_BASE64` | The keystore: `base64 -w0 upload.jks` |
| `ANDROID_KEYSTORE_PASSWORD` | Its password |
| `ANDROID_KEY_ALIAS` | The alias, `upload` with the command above |
| `ANDROID_KEY_PASSWORD` | That key's password |

Play wants each version code higher than the last. The run number starts at 1, so a
bundle uploaded by hand with a higher code would be in the way.

### Still to do for Google Play

In the repository:

- [x] Release builds use the production backend over HTTPS; debug builds use `10.0.2.2`
- [x] Release signing from the environment, nothing secret in the repository
- [x] `./gradlew :app:bundleRelease` builds a bundle
- [x] Version code from CI
- [x] A workflow that builds the signed bundle
- [x] `targetSdk` 36. Check the [target API level policy](https://support.google.com/googleplay/android-developer/answer/11926878)
  for what Play asks today. The app was not run on a phone after the move from 35.
- [x] Only foreground location: no `ACCESS_BACKGROUND_LOCATION` in the merged manifest
- [x] No `SCHEDULE_EXACT_ALARM` or `USE_EXACT_ALARM`: reminders are WorkManager jobs
- [x] Notifications are asked for when reminders are turned on, not at launch
- [x] OpenStreetMap attribution in Settings and on each place; the street map in the add
  form has MapLibre's own
- [x] Deleting the account: Settings → Delete my account, in the app and on the website
- [x] Privacy policy at `https://mangiamundo.com/privacy/`, linked from Settings
- [ ] The location question comes at first launch, with the welcome text behind it. An
  explanation before the question means asking only once "Use my current location" is
  tapped. Play demands this for background location, which the app does not use.
- [ ] The privacy policy names Nominatim but not the Overpass servers that "Look again"
  sends the position to, nor OpenFreeMap, which serves the map tiles in the add form.
- [ ] R8: off. See above.
- [ ] Uploading to a Play track from CI. It needs a service account of the Play Console.

In the Play Console:

- [ ] Developer account. A personal account made after November 2023 has to run a
  closed test with at least 12 testers for 14 days before it may publish to production.
- [ ] Create the app `app.mangiamundo` and enrol in Play App Signing with the first
  upload, which is done by hand
- [ ] Data safety form. From the privacy policy: approximate and precise location (used,
  not stored), app activity (check-ins, places added), personal info after signing in
  (email address, name, user id); encrypted in transit; deletion can be requested
- [ ] The account deletion link: `https://mangiamundo.com/privacy/` says how
- [ ] Content rating questionnaire, target audience (not children), ads (none)
- [ ] Listing: name, short description (80 characters), full description, 512×512 icon,
  1024×500 feature graphic, phone screenshots, category Food & Drink, contact email,
  website
- [ ] Internal testing, closed testing, then production with a staged rollout

## iOS

### A release build

The project is generated: `brew install xcodegen && xcodegen` in `mobile/ios`. The
version is `MARKETING_VERSION` in `project.yml`. The build number is
`CURRENT_PROJECT_VERSION`, 1 unless given on the command line:

```sh
xcodebuild archive -project Mangiamundo.xcodeproj -scheme Mangiamundo \
  -destination 'generic/platform=iOS' -archivePath build/Mangiamundo.xcarchive \
  CURRENT_PROJECT_VERSION=42
```

The app is for iPhone only (`TARGETED_DEVICE_FAMILY: "1"`). `PrivacyInfo.xcprivacy`
says what the app collects and that it uses UserDefaults; it has to say the same as the
privacy policy and the App Privacy answers in App Store Connect.

### From GitHub

Actions → **iOS release** → Run workflow (`.github/workflows/ios-release.yml`). It
archives the app, signs it for the App Store and uploads it to App Store Connect, where
it appears in TestFlight after processing. The build number is the run's number.

**This workflow has never run.** It was written without an Apple account or a Mac at
hand, so expect to fix it on its first runs. What to look at first:

- the `macos-26` runner and the Xcode on it: the App Store only takes builds made with
  a recent Xcode
- signing: the archive step makes a development certificate on each fresh runner, and
  an account can only hold a few of them
- whether the key's role is enough for automatic signing

Repository secrets, all from one App Store Connect API key (Users and Access →
Integrations → App Store Connect API). The workflow stops with a message naming the
ones that are missing.

| Secret | What |
| --- | --- |
| `ASC_KEY_ID` | The key's ID |
| `ASC_ISSUER_ID` | The issuer ID, shown above the list of keys |
| `ASC_KEY_P8` | The contents of the downloaded `AuthKey_<key id>.p8` |

### Still to do for the App Store

In the repository:

- [x] Privacy manifest
- [x] Build number from CI
- [x] iPhone only, set explicitly
- [x] `NSAllowsLocalNetworking` stays: it only allows plain HTTP to local addresses, and
  a release build talks to `https://mangiamundo.com` alone
- [x] Custom domain instead of `fly.dev`
- [x] Notifications are asked for when reminders are turned on, not at launch
- [x] OpenStreetMap attribution in Settings and on each place
- [x] Deleting the account in the app: Settings → Delete my account
- [x] Privacy policy, linked from Settings
- [x] A workflow that archives and uploads to TestFlight: written, never run
- [ ] The location question comes at first launch, as on Android

With an Apple account:

- [ ] Apple Developer Program membership
- [ ] Register the bundle id `app.mangiamundo`, with Sign in with Apple, and create the
  app record in App Store Connect
- [ ] The API key and the three secrets above
- [ ] App Privacy answers, the same as the privacy manifest: precise location, other
  user content (check-ins, places added), user id, and after signing in email address
  and name; all for app functionality, linked to the user, none for tracking
- [ ] Privacy policy URL and support URL in the listing
- [ ] Review notes: restaurants depend on the location, so name a city with good
  coverage, such as Leuven
- [ ] Age rating questionnaire
- [ ] Listing: name, subtitle (30 characters), description, keywords, 6.9" iPhone
  screenshots, category Food & Drink
- [ ] Internal TestFlight, external TestFlight (with a beta review), then App Review and
  a phased release
