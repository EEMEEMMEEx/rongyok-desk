<div align="center">
  <img src="icons/icon.png" alt="Rongyok Desk icon" width="96" height="96" />
  <h1>Rongyok Desk</h1>
  <p><strong>A dedicated single-site desktop browser for <a href="https://rongyok.com/">rongyok.com</a></strong></p>

  <p>
    <img src="https://img.shields.io/badge/version-v1.0.2-e4b65e?style=flat-square" alt="Version v1.0.2" />
    <img src="https://img.shields.io/badge/stack-Rust_%2B_Tauri_v2-7c5cb8?style=flat-square" alt="Rust + Tauri v2" />
    <img src="https://img.shields.io/badge/platforms-Windows_%7C_macOS_%7C_Linux-2ea44f?style=flat-square" alt="Platforms: Windows, macOS, Linux" />
    <a href="https://eemeemmeex.github.io/rongyok-desk/"><img src="https://img.shields.io/badge/landing_page-GitHub_Pages-1f6feb?style=flat-square" alt="Landing Page" /></a>
  </p>

  <p>
    <a href="https://github.com/EEMEEMMEEx/rongyok-desk/releases/download/untagged-64ea1f088167a8283cd3/Rongyok.Desk_1.0.2_x64-setup.exe"><img src="https://img.shields.io/badge/Download-Windows_.exe-0078D4?style=for-the-badge&logo=windows" alt="Download Windows .exe" /></a>
    <a href="https://github.com/EEMEEMMEEx/rongyok-desk/releases/download/untagged-64ea1f088167a8283cd3/Rongyok.Desk_1.0.2_x64_en-US.msi"><img src="https://img.shields.io/badge/Download-Windows_.msi-005A9E?style=for-the-badge&logo=windows" alt="Download Windows .msi" /></a>
    <a href="https://github.com/EEMEEMMEEx/rongyok-desk/releases/download/untagged-64ea1f088167a8283cd3/Rongyok.Desk_1.0.2_aarch64.dmg"><img src="https://img.shields.io/badge/Download-macOS_Apple_Silicon-111111?style=for-the-badge&logo=apple" alt="Download macOS Apple Silicon .dmg" /></a>
    <a href="https://github.com/EEMEEMMEEx/rongyok-desk/releases/download/untagged-64ea1f088167a8283cd3/Rongyok.Desk_1.0.2_x64.dmg"><img src="https://img.shields.io/badge/Download-macOS_Intel-333333?style=for-the-badge&logo=apple" alt="Download macOS Intel .dmg" /></a>
    <a href="https://github.com/EEMEEMMEEx/rongyok-desk/releases/download/untagged-64ea1f088167a8283cd3/Rongyok-Desk-1.0.2-x86_64.flatpak"><img src="https://img.shields.io/badge/Download-Linux_.flatpak-4A86CF?style=for-the-badge&logo=linux" alt="Download Linux .flatpak" /></a>
  </p>
</div>

Rongyok Desk opens **https://rongyok.com/** in a dedicated native window (`1280×800`, minimum `520×400`) so the site behaves like a standalone desktop app instead of a browser tab.

---

## Direct Downloads (`v1.0.2`)

| Platform | Architecture | Format | Size | Direct Download |
| --- | --- | --- | --- | --- |
| **Windows** | `x64` | NSIS Installer (`.exe`) — *Recommended* | 1.19 MB | [Rongyok.Desk_1.0.2_x64-setup.exe](https://github.com/EEMEEMMEEx/rongyok-desk/releases/download/untagged-64ea1f088167a8283cd3/Rongyok.Desk_1.0.2_x64-setup.exe) |
| **Windows** | `x64` | Windows Installer (`.msi`) | 1.70 MB | [Rongyok.Desk_1.0.2_x64_en-US.msi](https://github.com/EEMEEMMEEx/rongyok-desk/releases/download/untagged-64ea1f088167a8283cd3/Rongyok.Desk_1.0.2_x64_en-US.msi) |
| **macOS** | Apple Silicon (`arm64`) | Disk Image (`.dmg`) | 1.74 MB | [Rongyok.Desk_1.0.2_aarch64.dmg](https://github.com/EEMEEMMEEx/rongyok-desk/releases/download/untagged-64ea1f088167a8283cd3/Rongyok.Desk_1.0.2_aarch64.dmg) |
| **macOS** | Intel (`x64`) | Disk Image (`.dmg`) | 1.83 MB | [Rongyok.Desk_1.0.2_x64.dmg](https://github.com/EEMEEMMEEx/rongyok-desk/releases/download/untagged-64ea1f088167a8283cd3/Rongyok.Desk_1.0.2_x64.dmg) |
| **Linux** | `x86_64` | Flatpak Bundle (`.flatpak`) | 1.10 MB | [Rongyok-Desk-1.0.2-x86_64.flatpak](https://github.com/EEMEEMMEEx/rongyok-desk/releases/download/untagged-64ea1f088167a8283cd3/Rongyok-Desk-1.0.2-x86_64.flatpak) |

### Preview

<p align="center">
  <img src="shots/now.png" alt="Rongyok Desk window showing rongyok.com" width="760" />
</p>

---

## Installation Notes

> **Unsigned Builds Notice:** The `v1.0.2` installers are currently unsigned. Windows SmartScreen and macOS Gatekeeper will show a first-run warning until code-signing certificates are configured.

- **Windows (`.exe` / `.msi`)**
  1. Download and run [Rongyok.Desk_1.0.2_x64-setup.exe](https://github.com/EEMEEMMEEx/rongyok-desk/releases/download/untagged-64ea1f088167a8283cd3/Rongyok.Desk_1.0.2_x64-setup.exe) (per-user installer; automatically downloads Microsoft Edge WebView2 Runtime if missing) or [Rongyok.Desk_1.0.2_x64_en-US.msi](https://github.com/EEMEEMMEEx/rongyok-desk/releases/download/untagged-64ea1f088167a8283cd3/Rongyok.Desk_1.0.2_x64_en-US.msi).
  2. If **Windows SmartScreen** appears on first launch, click **More info** → **Run anyway**.

- **macOS (`10.15+`)**
  1. Download [Rongyok.Desk_1.0.2_aarch64.dmg](https://github.com/EEMEEMMEEx/rongyok-desk/releases/download/untagged-64ea1f088167a8283cd3/Rongyok.Desk_1.0.2_aarch64.dmg) (Apple Silicon) or [Rongyok.Desk_1.0.2_x64.dmg](https://github.com/EEMEEMMEEx/rongyok-desk/releases/download/untagged-64ea1f088167a8283cd3/Rongyok.Desk_1.0.2_x64.dmg) (Intel).
  2. Open the `.dmg` and drag **Rongyok Desk.app** into **Applications**.
  3. On first launch, bypass **macOS Gatekeeper** by **Control-clicking (right-clicking)** the app → **Open** → **Open** (or allow it via **System Settings → Privacy & Security → Open Anyway**).

- **Linux (`x86_64` Flatpak)**
  ```bash
  flatpak remote-add --user --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
  flatpak install --user Rongyok-Desk-1.0.2-x86_64.flatpak
  flatpak run com.rongyok.ssb
  ```

---

## Key Features

- **One Window, One Site** — Opens directly at `https://rongyok.com/` in a clean `1280×800` native window with persistent cookies, cache, and `localStorage` stored on disk (`<app_local_data_dir>/webview`; `~/.var/app/com.rongyok.ssb` on Flatpak) so logins survive restarts.
- **Strict Domain Isolation** — Every navigation (`on_navigation`) and pop-up / `target="_blank"` request (`on_new_window`) is checked before loading. Only `http://` and `https://` URLs on `rongyok.com` or `*.rongyok.com` (plus internal `about:`, `data:`, `blob:` webview URLs) load inside the window.
- **External Link Handoff** — Links to other domains, lookalike hosts, or external schemes (`mailto:`, `tel:`, `file:`) automatically open in your default system browser (`ShellExecuteW` on Windows, `open` on macOS, `xdg-open` on Linux) while keeping Rongyok Desk on the site.
- **Remembered Window Placement** — Saves `x y width height maximized` to `window-placement.txt` on close/destroy. Validates usable bounds (`520..20000 × 400..20000`) and checks `available_monitors()` on startup, falling back to a centered window on the primary monitor if a display was disconnected.
- **HTML5 Media & Fullscreen** — Enables autoplay without prior gesture (`--autoplay-policy=no-user-gesture-required`) and disables background throttling. On Windows WebView2, `ContainsFullScreenElementChanged` syncs HTML5 video fullscreen with native window fullscreen; Flatpak enables `--socket=pulseaudio` and `--device=dri`.
- **Resilient Keyboard Shortcuts & Menu** — Capture-phase JS listeners prevent page scripts from swallowing navigation keys, paired with a native **Navigate** menu.
- **Production Hygiene** — DevTools are enabled only in debug builds (`cfg!(debug_assertions)`) and disabled in release builds.

### Keyboard Shortcuts & Menu

| Action | Shortcut | Notes |
| --- | --- | --- |
| Back | `Alt+Left` | Handled at capture phase + **Navigate** menu |
| Forward | `Alt+Right` | Handled at capture phase + **Navigate** menu |
| Reload | `F5` / `Ctrl+R` / `Cmd+R` | Reloads current page |
| Go to `rongyok.com` | `Ctrl+H` | Returns to `START_URL` (`https://rongyok.com/`) |
| Full screen | `F11` | Toggles native window fullscreen |
| Open this page in browser | Menu only | Opens current URL in system browser |
| Quit | `Ctrl+Q` | Closes the application |

