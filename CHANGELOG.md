# Changelog

All notable changes to **Rongyok Desk** are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.3.1] - 2026-11-16

### Added
- Add `auto-tag.yml` workflow to automatically tag `v<version>` and trigger `release.yml` when `tauri.conf.json` version changes on `main`.

### Fixed
- Clean cached `release/bundle` outputs before Windows and macOS CI builds so stale installers from prior Cargo cache hits are never re-uploaded.

## [1.3.0] - 2026-11-16

### Added
- Support automatic release-note extraction from `CHANGELOG.md` into `latest.json` and GitHub Releases.
- Add bilingual Thai / English switcher (`TH` / `EN`) on the GitHub Pages landing site (`docs/`).
- Add opt-in CI code-signing and notarization hooks for Windows Authenticode and macOS Developer ID.
- Enable in-app `tauri-plugin-updater` updates on Developer-ID-signed and notarized macOS builds (`RONGYOK_MACOS_SIGNED=1`), keeping direct `.dmg` download fallback when unsigned.

### Changed
- Sync AppStream `<releases>` metadata in `flatpak/com.rongyok.ssb.metainfo.xml`.

## [1.2.0] - 2026-11-16

### Added
- **In-app update checker (Phase 1):** Added `Help > Check for Updates...` and a silent startup version check against `latest.json` using `reqwest` (`native-tls`, `json`, `system-proxy`) and `semver`, with strict `https://github.com/EEMEEMMEEx/rongyok-desk/` allowlist validation and an isolated closed Shadow DOM modal.
- **Signed updater support (Phase 2):** Enabled `bundle.createUpdaterArtifacts: true` with an Ed25519 signing key and gated `tauri-plugin-updater` v2 to Windows and macOS targets (`Linux` Flatpak excluded) with in-app install on Windows and browser download fallback.

### Security
- Protected `rongyok-desk://` update modal actions with a per-launch 128-bit cryptographic nonce, custom redirect hop allowlist, and a 1 MiB manifest size cap.

## [1.1.0] - 2026-11-16

### Added
- First-launch onboarding shortcut reference modal (`Help > Keyboard Shortcuts`, `F1` / `Ctrl+/`) persisted locally via `onboarding-seen.txt`.
- `Help > About Rongyok Desk` modal displaying version, homepage link, copyright, and one-click `Copy Version Info`.

### Fixed
- Unified GitHub Release publishing and `latest.json` generation into a single `publish-release` workflow job and fixed `releases/latest/download/` links.

## [1.0.2] - 2026-11-15

### Added
- Static GitHub Pages landing page (`app/docs/`) and deployment workflow (`pages.yml`).
- Bilingual `README.md` and `README.th.md` installer documentation.

### Fixed
- Configured `macos-15` runner and GNOME 49 / `rust-stable` 25.08 Flatpak runtime in CI.

## [1.0.1] - 2026-11-15

### Added
- Initial Rongyok Desk desktop wrapper built with Rust and Tauri v2 (`wry`), strict `rongyok.com` domain isolation, external browser handoff, remembered window placement, capture-phase shortcuts, and `--screenshot` preview capture on Windows.
