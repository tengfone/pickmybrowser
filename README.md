# PickMyBrowser

Pick which browser opens every link. Set PickMyBrowser as your default browser and each link you click opens a small picker: press a number, done.

![PickMyBrowser demo](assets/demo.gif)

Works on **Windows**, **macOS** and **Linux**.

## Features

### ⚡ Fast picking
- **One keypress.** Each browser gets a number from `1` to `9`. You can also use the arrow keys or click.
- **Private windows.** Hold `Shift` while picking to open the link in a private or incognito window.
- **Profiles.** Press `→` on a browser to list its profiles (Chrome, Edge, Brave, Firefox and more), then press the profile's letter.
- **Type to filter.** If you have more than nine browsers, start typing to narrow the list.

### 🎯 Rules
- **Send links automatically.** Patterns like `*.figma.com` or `github.com/work/*` go straight to the browser and profile you choose.
- **Your choice of timing.** A rule can wait for a short countdown (press `Space` to cancel) or open immediately.

### 🛡️ Safer links
- **Tracking removal.** Parameters such as `utm_*` and `fbclid` are removed before the link opens.
- **Lookalike warnings.** Domains such as `paypa1.com` are flagged, and rules never open them automatically.

### 🪶 Light and out of the way
- **Small.** The app is about 4 MB, and the Windows installer is about 1.6 MB.
- **Only runs when needed.** It sits in the tray and runs only when you start it or click a link.
- **Automatic browser detection.** Installed browsers are found with their real icons.
- **Recent links.** The picker keeps a list of links you opened recently.
- **Your look.** Choose a light or dark theme, and show the picker at the cursor or in the centre of the screen.

### ⌨️ Keyboard shortcuts

| Key | Action |
| --- | --- |
| `1`–`9` | Open in that browser |
| `↑` / `↓`, `Enter` | Move through the list and open the selected browser |
| `Shift` + pick | Open in a private window |
| `→` | Show the browser's profiles |
| `Space` | Cancel a rule's countdown |
| Type | Filter the list (more than 9 browsers) |

## Install

Download from [Releases](https://github.com/tengfone/pickmybrowser/releases/latest):

| Platform | File |
| --- | --- |
| Windows | `PickMyBrowser-Setup-x.y.z.exe` (per-user, no admin) |
| macOS Apple Silicon / Intel | `PickMyBrowser-x.y.z-mac-arm64.zip` / `-mac-x64.zip` → move to Applications |
| Debian / Ubuntu | `PickMyBrowser-x.y.z-amd64.deb` |
| Fedora / openSUSE | `PickMyBrowser-x.y.z-x86_64.rpm` |
| Other Linux (advanced) | `PickMyBrowser-x.y.z.AppImage` |

Builds are unsigned. macOS: right-click the app → Open the first time. Windows: SmartScreen → More info → Run anyway.

Then open PickMyBrowser and press **Set as default** (or choose it in your system's default-browser settings).

## Uninstall

Nothing is left behind: settings, cached icons, launch-at-login entries and the browser registration are all removed.

- **Windows:** Settings → Apps → PickMyBrowser → Uninstall.
- **macOS:** PickMyBrowser Settings → General → **Remove PickMyBrowser**, then drag the app to the Bin.
- **Linux:** `sudo apt remove pickmybrowser` / `sudo dnf remove pickmybrowser`.

## Development

Needs Node (see `.nvmrc`) and Rust (see `src-tauri/rust-toolchain.toml`); on Linux also the [Tauri system packages](https://tauri.app/start/prerequisites/).

```sh
npm ci
npm run tauri dev          # run
npm run tauri build        # installers in src-tauri/target/release/bundle
npm run test:e2e           # build the E2E app and run the WebdriverIO suite
npm run test:smoke         # release-binary smoke test (after `npx tauri build --no-bundle`)
npm run demo               # re-record assets/demo.gif (needs ffmpeg + Edge/Chrome; after build:e2e)
```

The E2E build (`--features wdio`, `PMB_E2E=1`) uses a throwaway config folder and a fixed browser list, and records launches instead of opening browsers, so it never touches your real setup.

Releasing: `npm run release:patch|minor|major` bumps the version, tags and pushes; CI tests on all three OSes, builds every platform and creates a draft release.

## License

[MIT](LICENSE)
