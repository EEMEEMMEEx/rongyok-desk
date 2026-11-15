# Rongyok Desk — a dedicated single-site browser

A small desktop window that shows **https://rongyok.com/** and stays there, so the
site feels like an app instead of a tab in a browser.

## What it does

- **One window, one site.** It opens straight on rongyok.com at 1280×800 with a
  normal window title, and remembers where you signed in between launches,
  because its cookies and storage live in a stable folder on disk.
- **It reopens where you left it.** Move or resize the window and it comes back the
  same size and in the same spot next time, including maximized. If the screen it
  was on is no longer plugged in, it falls back to the middle of the main screen
  instead of opening out of sight.
- **Stays on the site.** Every navigation is checked before it happens. Addresses
  on `rongyok.com` and any `*.rongyok.com` subdomain are allowed. Anything else
  opens in your normal web browser instead, and this window stays where it was.
  Pop-ups and `target="_blank"` links follow the same rule.
- **Media works.** Video and audio play without needing a click first, and when a
  video asks for full screen the whole window goes full screen (and comes back).
- **Browser keys work.** Alt+Left / Alt+Right go back and forward, F5 or Ctrl+R
  reloads. They are handled by the window itself, so a page can't swallow them.
  There is also a small **Navigate** menu with the same actions, plus
  "Open this page in browser" and F11 for full screen.
- **No page inspector** in a normal build; it is only available in debug builds.

## Building and running

```
cargo build --release
target\release\rongyok-ssb.exe
```

## Building the installers

The bundle settings live in `tauri.conf.json`, under `bundle`. Four installer
formats are configured:

| Format | Made on | File |
| --- | --- | --- |
| NSIS installer | Windows | `Rongyok Desk_1.0.2_x64-setup.exe` |
| MSI installer | Windows | `Rongyok Desk_1.0.2_x64_en-US.msi` |
| Disk image | macOS | `Rongyok Desk_1.0.2_<arch>.dmg` |
| Flatpak | Linux | `Rongyok-Desk-1.0.2-x86_64.flatpak` |

The Windows and macOS ones come from the Tauri command line tool:

```
cargo install tauri-cli --version "^2" --locked
cargo tauri build
```

Each format has to be built on the system it is for, which is why the automated
build uses one machine per system. The Flatpak is separate because Flatpak is
its own packaging system: see below.

The icons in `icons/` are all referenced by `tauri.conf.json`, and each one is
needed: `icon.ico` for Windows, `icon.icns` for macOS, and the PNGs for Linux
and for the window itself. `tools/make_icon` draws all of them in the same
style, so they can be regenerated at any time with:

```
cargo run --manifest-path tools/make_icon/Cargo.toml
```

## Building the Flatpak

The Flatpak lives in `flatpak/`: the manifest `com.rongyok.ssb.yml` plus a menu
entry and the metadata software centres read. On a Linux machine with `flatpak`
and `flatpak-builder` installed:

```
./flatpak/build-flatpak.sh
```

That writes `dist/flatpak/Rongyok-Desk-<version>-x86_64.flatpak`, which installs with:

```
flatpak install --user dist/flatpak/Rongyok-Desk-1.0.2-x86_64.flatpak
flatpak run com.rongyok.ssb
```

The script checks for the tools and the GNOME runtime and SDK it builds
against, and tells you the command to install anything missing.

## Automatic releases

`.github/workflows/release.yml` builds all four installers on GitHub's machines:
Windows for the `.exe` and `.msi`, an Apple Silicon and an Intel Mac for the
`.dmg`, and Ubuntu for the `.flatpak`. Push a tag such as `v1.0.0` to publish a
draft release with all four attached, or run the workflow by hand from the
Actions tab to get them as downloads without publishing.

The workflow finds the folder holding `tauri.conf.json` by itself, so it works
whether `app/` is the top of your repository or one level down.

## Taking a picture of the window

```
target\release\rongyok-ssb.exe --screenshot shots\now.png
```

This starts the window **without taking focus** (and on Windows behind other
windows), waits for the page to paint, saves a PNG, and exits. It exits with code
0 when it saved a picture with visible content, and 1 when it could only produce a
blank one. The `--screenshot=<file>` form works too.

## Tests

```
cargo test
```

Six tests cover the address rules: the site and its subdomains pass, and other
sites, lookalikes such as `rongyok.com.evil.example` or `fakerongyok.com`, and
`mailto:` links are blocked. Four more cover the remembered window spot: it
round-trips, a maximized window is noted, a damaged note is ignored, and an
impossible size is refused.

## Notes and limits

- The picture option uses the Windows webview's own capture call. On macOS and
  Linux that call does not exist, so `--screenshot` cannot produce a picture there.
- `https://rongyok.com/` is behind Cloudflare, which answers some networks and
  regions with a "you have been blocked" page. The app shows whatever the site
  returns; it does not try to get around that.
- Only files in this folder belong to the app. `tools/make_icon` is a one-off
  helper that draws the icon set in `icons/`.
- The installers are unsigned. Windows SmartScreen and macOS Gatekeeper will warn
  the first time someone opens them until the packages are code signed, which
  needs a certificate from each platform's authority. No certificate is included
  here, and the workflow does not pass any signing secrets; add them to the
  `tauri-action` steps when you have them.
- The Flatpak builds against the GNOME 46 runtime. Change `runtime-version` in
  the manifest (and the same version in the workflow) to build against a newer
  one.