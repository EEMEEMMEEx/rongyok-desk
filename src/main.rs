#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

// Rongyok Desk — a dedicated single-site browser.
//
// One native window that shows https://rongyok.com/ and stays there: links that
// leave the site open in the person's normal browser instead, and the usual
// browser keys (Back, Forward, Reload) keep working.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::utils::config::BackgroundThrottlingPolicy;
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// Authoritative app identity and version metadata compiled from Cargo.
const APP_NAME: &str = "Rongyok Desk";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const APP_COPYRIGHT_JS: &str = r"Copyright \u00a9 2026 Rongyok";
const APP_DESCRIPTION: &str = "A dedicated single-site desktop browser for rongyok.com.";
/// The site this window is locked to.
const START_URL: &str = "https://rongyok.com/";
/// The session/cookies live beside this, so signing in survives a restart.
const WINDOW_LABEL: &str = "main";
/// Marker file beside window-placement.txt tracking first-launch onboarding dismissal.
const ONBOARDING_STATE_FILE: &str = "onboarding-seen.txt";
/// Custom-scheme navigation intercepted by Rust to persist onboarding dismissal immediately.
const ONBOARDING_DISMISS_SCHEME: &str = "rongyok-desk";
const ONBOARDING_DISMISS_HOST: &str = "onboarding-dismissed";

/// Build the diagnostic version string copied from the About dialog.
fn version_info_string() -> String {
    format!(
        "{APP_NAME} v{APP_VERSION} ({}/{})",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

/// True when a URL belongs to the site we host: rongyok.com or any subdomain of it.
fn is_internal(url: &Url) -> bool {
    match url.scheme() {
        // Pages the webview makes itself: the starting about:blank, media blobs,
        // inlined images. They can't take the person off the site.
        "about" | "data" | "blob" => true,
        "http" | "https" => match url.host_str() {
            Some(host) => {
                let host = host.to_ascii_lowercase();
                host == "rongyok.com" || host.ends_with(".rongyok.com")
            }
            None => false,
        },
        // Anything else (file:, mailto: handled below, custom app schemes) is foreign.
        _ => false,
    }
}

/// Hand a link to the person's normal browser, so it opens outside this window.
fn open_in_system_browser(target: &str) {
    #[cfg(target_os = "windows")]
    {
        use windows::core::{HSTRING, PCWSTR};
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        let operation = HSTRING::from("open");
        let file = HSTRING::from(target);
        unsafe {
            // A failure here simply means nothing opened; the window stays on the site.
            let _ = ShellExecuteW(
                None,
                PCWSTR(operation.as_ptr()),
                PCWSTR(file.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            );
        }
    }

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(target).spawn();
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(target).spawn();
    }
}

/// Where the window was last time: so reopening it feels like a real app rather
/// than starting from scratch each launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WindowPlacement {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    maximized: bool,
}

impl WindowPlacement {
    /// A saved placement is only worth honouring if it is sane: a real size, and
    /// not so tiny the person could lose the window.
    fn is_usable(&self) -> bool {
        self.width >= 520 && self.height >= 400 && self.width <= 20000 && self.height <= 20000
    }
}

/// Read a placement back from the one-line form `x y width height maximized`.
fn parse_placement(text: &str) -> Option<WindowPlacement> {
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() != 5 {
        return None;
    }
    let placement = WindowPlacement {
        x: parts[0].parse().ok()?,
        y: parts[1].parse().ok()?,
        width: parts[2].parse().ok()?,
        height: parts[3].parse().ok()?,
        maximized: match parts[4] {
            "1" | "true" => true,
            "0" | "false" => false,
            _ => return None,
        },
    };
    placement.is_usable().then_some(placement)
}

fn format_placement(placement: &WindowPlacement) -> String {
    format!(
        "{} {} {} {} {}",
        placement.x,
        placement.y,
        placement.width,
        placement.height,
        if placement.maximized { 1 } else { 0 }
    )
}

/// Where the saved placement lives, beside the webview's own data folder.
fn placement_file(handle: &AppHandle) -> Option<PathBuf> {
    handle
        .path()
        .app_local_data_dir()
        .ok()
        .map(|folder| folder.join("window-placement.txt"))
}

fn load_placement(handle: &AppHandle) -> Option<WindowPlacement> {
    let path = placement_file(handle)?;
    let text = std::fs::read_to_string(path).ok()?;
    parse_placement(&text)
}

fn save_placement(handle: &AppHandle, placement: &WindowPlacement) {
    let Some(path) = placement_file(handle) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, format_placement(placement));
}

/// Interpret the contents of the onboarding marker file.
fn parse_onboarding_seen(text: &str) -> bool {
    matches!(
        text.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "seen" | "yes"
    )
}

fn read_onboarding_seen(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .map(|text| parse_onboarding_seen(&text))
        .unwrap_or(false)
}

fn write_onboarding_seen(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, "1\n")
}

/// Where the first-launch onboarding state lives, beside `window-placement.txt`.
fn onboarding_file(handle: &AppHandle) -> Option<PathBuf> {
    handle
        .path()
        .app_local_data_dir()
        .ok()
        .map(|folder| folder.join(ONBOARDING_STATE_FILE))
}

fn has_seen_onboarding(handle: &AppHandle) -> bool {
    onboarding_file(handle)
        .as_deref()
        .map(read_onboarding_seen)
        .unwrap_or(false)
}

fn mark_onboarding_seen(handle: &AppHandle) {
    if let Some(path) = onboarding_file(handle) {
        let _ = write_onboarding_seen(&path);
    }
}

/// First-launch onboarding only auto-opens in normal interactive launches.
fn should_show_onboarding(taking_picture: bool, seen: bool) -> bool {
    !taking_picture && !seen
}

/// Sentinel URL sent by the Shadow DOM modal when the user dismisses first-launch onboarding.
fn is_onboarding_dismiss_url(url: &Url) -> bool {
    url.scheme() == ONBOARDING_DISMISS_SCHEME && url.host_str() == Some(ONBOARDING_DISMISS_HOST)
}

/// Auto-open onboarding only after an internal HTTP/HTTPS page finishes loading.
fn should_auto_open_on_page_load(
    event: PageLoadEvent,
    url: &Url,
    onboarding_pending: bool,
) -> bool {
    onboarding_pending
        && matches!(event, PageLoadEvent::Finished)
        && matches!(url.scheme(), "http" | "https")
        && is_internal(url)
}

/// True when this point still falls on a screen that is plugged in now, so a
/// window saved on a monitor that has since been unplugged isn't lost off-screen.
fn position_is_on_a_screen(handle: &AppHandle, placement: &WindowPlacement) -> bool {
    let Ok(monitors) = handle.available_monitors() else {
        return true;
    };
    if monitors.is_empty() {
        return true;
    }
    let mut on_screen = false;
    for monitor in monitors {
        let origin = monitor.position();
        let size = monitor.size();
        let left = origin.x;
        let top = origin.y;
        let right = left.saturating_add(size.width as i32);
        let bottom = top.saturating_add(size.height as i32);
        // Keep a decent strip of the title bar reachable.
        if placement.x >= left - 8
            && placement.y >= top - 8
            && placement.x < right - 100
            && placement.y < bottom - 60
        {
            on_screen = true;
            break;
        }
    }
    on_screen
}

/// Tell a JPEG/PNG apart from a blank one by looking at the spread of colours.
fn png_looks_blank(bytes: &[u8]) -> bool {
    let decoder = png::Decoder::new(bytes);
    let mut reader = match decoder.read_info() {
        Ok(r) => r,
        Err(_) => return true,
    };
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = match reader.next_frame(&mut buffer) {
        Ok(i) => i,
        Err(_) => return true,
    };
    if info.width == 0 || info.height == 0 {
        return true;
    }

    let channels = match info.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return false,
    };

    let pixels = buffer.len() / channels;
    if pixels == 0 {
        return true;
    }

    let stride = (pixels / 4000).max(1);
    let mut low = 255u8;
    let mut high = 0u8;
    for index in (0..pixels).step_by(stride) {
        let wanted = match channels {
            1 => &buffer[index..index + 1],
            2 => &buffer[index * 2..index * 2 + 1],
            3 => &buffer[index * 3..index * 3 + 3],
            _ => &buffer[index * 4..index * 4 + 3],
        };
        for channel in wanted {
            low = low.min(*channel);
            high = high.max(*channel);
        }
    }

    // A flat one-colour picture (all white, all black) is blank.
    high.saturating_sub(low) < 8
}

// The fullscreen handler has to stay reachable for as long as the webview lives,
// and the WebView2 COM object refuses to cross threads, so it is parked here.
#[cfg(target_os = "windows")]
thread_local! {
    static FULLSCREEN_HANDLER: std::cell::RefCell<Option<
        webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2ContainsFullScreenElementChangedEventHandler,
    >> = const { std::cell::RefCell::new(None) };
}

/// Keep a note of where the window is while it is used, so the next launch can put
/// it back. Saving on every move would be wasteful, so it is written when the
/// window is closed, keeping the most recent size and position seen.
fn remember_placement(handle: &AppHandle, window: &WebviewWindow) {
    use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Latest {
        x: AtomicI32,
        y: AtomicI32,
        width: AtomicU32,
        height: AtomicU32,
    }

    let latest = Arc::new(Latest::default());
    let latest_for_events = Arc::clone(&latest);
    let last_maximized = Arc::new(Mutex::new(false));
    let last_maximized_for_events = Arc::clone(&last_maximized);

    let handle_for_events = handle.clone();
    window.on_window_event(move |event| {
        use tauri::WindowEvent;

        // Only a settled, normal window tells us the size and spot worth restoring;
        // a maximized window reports the full screen instead.
        if matches!(event, WindowEvent::Resized(_) | WindowEvent::Moved(_)) {
            let Some(window) = handle_for_events.get_webview_window(WINDOW_LABEL) else {
                return;
            };
            let maximized = window.is_maximized().unwrap_or(false);
            if let Ok(mut flag) = last_maximized_for_events.lock() {
                *flag = maximized;
            }
            if maximized {
                return;
            }
            if let (Ok(position), Ok(size)) = (window.outer_position(), window.outer_size()) {
                latest_for_events.x.store(position.x, Ordering::Relaxed);
                latest_for_events.y.store(position.y, Ordering::Relaxed);
                latest_for_events.width.store(size.width, Ordering::Relaxed);
                latest_for_events.height.store(size.height, Ordering::Relaxed);
            }
        }

        if matches!(event, WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed) {
            let placement = WindowPlacement {
                x: latest_for_events.x.load(Ordering::Relaxed),
                y: latest_for_events.y.load(Ordering::Relaxed),
                width: latest_for_events.width.load(Ordering::Relaxed),
                height: latest_for_events.height.load(Ordering::Relaxed),
                maximized: last_maximized_for_events
                    .lock()
                    .map(|flag| *flag)
                    .unwrap_or(false),
            };
            // A window that never reported a size keeps the previous note.
            if placement.is_usable() {
                let handle = handle_for_events.clone();
                save_placement(&handle, &placement);
            }
        }
    });
}

/// Watch for the page asking for full screen (a video going fullscreen) and follow
/// it with the native window, so the video really fills the screen.
#[cfg(target_os = "windows")]
fn watch_video_fullscreen(window: &WebviewWindow) {
    use webview2_com::ContainsFullScreenElementChangedEventHandler;
    use windows::core::BOOL;

    let target = window.clone();

    let _ = window.with_webview(move |platform| unsafe {
        let controller = platform.controller();
        let webview = match controller.CoreWebView2() {
            Ok(webview) => webview,
            Err(_) => return,
        };

        let target_for_event = target.clone();
        let handler =
            ContainsFullScreenElementChangedEventHandler::create(Box::new(move |sender, _args| {
                let mut wanted_flag = BOOL(0);
                let wanted = match &sender {
                    Some(view) => {
                        let _ = view.ContainsFullScreenElement(&mut wanted_flag);
                        wanted_flag.as_bool()
                    }
                    None => false,
                };
                // Only change the window when the wish actually differs, so a page
                // that sets this repeatedly doesn't fight the person.
                if let Ok(current) = target_for_event.is_fullscreen() {
                    if current != wanted {
                        let _ = target_for_event.set_fullscreen(wanted);
                    }
                }
                Ok(())
            }));

        let mut token = 0i64;
        if webview
            .add_ContainsFullScreenElementChanged(&handler, &mut token)
            .is_ok()
        {
            FULLSCREEN_HANDLER.with(|slot| {
                *slot.borrow_mut() = Some(handler);
            });
        }
    });
}

#[cfg(not(target_os = "windows"))]
fn watch_video_fullscreen(_window: &WebviewWindow) {}

/// The keys and shortcuts dialog a browser window keeps working, whatever the
/// page does with them. Runs before site scripts, isolates its modal inside a
/// closed Shadow DOM, and notifies Rust over `rongyok-desk://onboarding-dismissed`
/// when first-launch onboarding is dismissed.
const SHORTCUT_SCRIPT: &str = r#"
(function () {
  if (window.__rongyokKeys) return;
  window.__rongyokKeys = true;

  var DISMISS_URL = 'rongyok-desk://onboarding-dismissed';
  var HOST_ID = 'rongyok-desk-shortcuts-host';
  var modalOpen = false;
  var onboardingActive = false;
  var lastFocused = null;
  var ui = null;

  var SVG_NS = 'http://www.w3.org/2000/svg';

  function createSvgIcon(paths, size) {
    var svg = document.createElementNS(SVG_NS, 'svg');
    svg.setAttribute('viewBox', '0 0 24 24');
    svg.setAttribute('width', String(size));
    svg.setAttribute('height', String(size));
    svg.setAttribute('fill', 'none');
    svg.setAttribute('stroke', 'currentColor');
    svg.setAttribute('stroke-width', '1.85');
    svg.setAttribute('stroke-linecap', 'round');
    svg.setAttribute('stroke-linejoin', 'round');
    svg.setAttribute('aria-hidden', 'true');
    for (var i = 0; i < paths.length; i++) {
      var el = document.createElementNS(SVG_NS, paths[i][0]);
      var attrs = paths[i][1];
      for (var k in attrs) {
        if (Object.prototype.hasOwnProperty.call(attrs, k)) {
          el.setAttribute(k, attrs[k]);
        }
      }
      svg.appendChild(el);
    }
    return svg;
  }

  function buildKeyCombo(parts) {
    var wrap = document.createElement('span');
    wrap.className = 'keys';
    for (var i = 0; i < parts.length; i++) {
      var part = parts[i];
      if (part.type === 'kbd') {
        var kbd = document.createElement('kbd');
        kbd.textContent = part.text;
        wrap.appendChild(kbd);
      } else if (part.type === 'sep') {
        var sep = document.createElement('span');
        sep.className = 'sep';
        sep.textContent = part.text;
        wrap.appendChild(sep);
      } else {
        var muted = document.createElement('span');
        muted.className = 'menu-only';
        muted.textContent = part.text;
        wrap.appendChild(muted);
      }
    }
    return wrap;
  }

  function ensureUi() {
    if (ui) {
      var rootTarget = document.documentElement || document.body;
      if (rootTarget && !ui.host.isConnected) {
        rootTarget.appendChild(ui.host);
      }
      return ui;
    }

    var rootTarget = document.documentElement || document.body;
    if (!rootTarget) return null;

    var host = document.createElement('div');
    host.id = HOST_ID;

    var shadow = host.attachShadow({ mode: 'closed' });

    var style = document.createElement('style');
    style.textContent = [
      ':host {',
      '  all: initial !important;',
      '  position: fixed !important;',
      '  inset: 0 !important;',
      '  width: 100vw !important;',
      '  height: 100vh !important;',
      '  margin: 0 !important;',
      '  padding: 0 !important;',
      '  border: 0 !important;',
      '  background: transparent !important;',
      '  z-index: 2147483647 !important;',
      '  isolation: isolate !important;',
      '  display: none !important;',
      '  pointer-events: none !important;',
      '  font-family: "Segoe UI", system-ui, -apple-system, sans-serif !important;',
      '  color: #f5f2fa !important;',
      '}',
      ':host([data-rongyok-open="true"]) {',
      '  display: block !important;',
      '  pointer-events: auto !important;',
      '}',
      '*, *::before, *::after { box-sizing: border-box; }',
      'svg { pointer-events: none; }',
      '.backdrop {',
      '  position: fixed; inset: 0;',
      '  display: flex; align-items: center; justify-content: center;',
      '  padding: 12px;',
      '  background: rgba(12, 9, 18, 0.78);',
      '  backdrop-filter: blur(6px);',
      '  -webkit-backdrop-filter: blur(6px);',
      '}',
      '.dialog {',
      '  width: min(620px, calc(100vw - 24px));',
      '  max-height: calc(100vh - 24px);',
      '  overflow-y: auto;',
      '  border-radius: 16px;',
      '  background: linear-gradient(180deg, #1b1526 0%, #120e18 100%);',
      '  border: 1px solid rgba(228, 182, 94, 0.36);',
      '  box-shadow: 0 24px 64px rgba(0, 0, 0, 0.68), 0 0 0 1px rgba(255, 255, 255, 0.04) inset;',
      '  color: #f5f2fa;',
      '  padding: 16px 18px;',
      '}',
      '.header {',
      '  display: flex; align-items: flex-start; justify-content: space-between; gap: 12px;',
      '  margin-bottom: 14px;',
      '}',
      '.title-wrap { display: flex; align-items: flex-start; gap: 12px; min-width: 0; }',
      '.icon-badge {',
      '  flex: 0 0 auto;',
      '  width: 36px; height: 36px;',
      '  border-radius: 10px;',
      '  display: inline-flex; align-items: center; justify-content: center;',
      '  background: rgba(228, 182, 94, 0.14);',
      '  border: 1px solid rgba(228, 182, 94, 0.38);',
      '  color: #e4b65e;',
      '}',
      '.eyebrow {',
      '  margin: 0 0 2px;',
      '  font-size: 10.5px; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase;',
      '  color: #e4b65e;',
      '}',
      'h2 {',
      '  margin: 0;',
      '  font-size: 18px; font-weight: 700; line-height: 1.25; color: #f5f2fa;',
      '}',
      '.subtitle {',
      '  margin: 3px 0 0;',
      '  font-size: 12.5px; line-height: 1.4; color: #cfc6de;',
      '}',
      '.close-btn {',
      '  flex: 0 0 auto;',
      '  width: 32px; height: 32px;',
      '  border-radius: 8px;',
      '  border: 1px solid rgba(255, 255, 255, 0.12);',
      '  background: rgba(255, 255, 255, 0.04);',
      '  color: #cfc6de;',
      '  display: inline-flex; align-items: center; justify-content: center;',
      '  cursor: pointer;',
      '  transition: background 140ms ease, border-color 140ms ease, color 140ms ease;',
      '}',
      '.close-btn:hover { background: rgba(228, 182, 94, 0.14); border-color: rgba(228, 182, 94, 0.4); color: #f5f2fa; }',
      '.close-btn:focus-visible, .primary-btn:focus-visible {',
      '  outline: 2px solid #f0c674; outline-offset: 2px;',
      '}',
      '.table-wrap {',
      '  border-radius: 12px;',
      '  border: 1px solid rgba(255, 255, 255, 0.1);',
      '  background: rgba(18, 14, 24, 0.72);',
      '  overflow-x: auto;',
      '  overflow-y: hidden;',
      '}',
      'table { width: 100%; border-collapse: collapse; font-size: 12.5px; }',
      'thead th {',
      '  text-align: left;',
      '  font-size: 10.5px; font-weight: 600; letter-spacing: 0.06em; text-transform: uppercase;',
      '  color: #b7aec7;',
      '  padding: 9px 12px;',
      '  background: rgba(255, 255, 255, 0.03);',
      '  border-bottom: 1px solid rgba(255, 255, 255, 0.1);',
      '}',
      'tbody td {',
      '  padding: 8px 12px;',
      '  border-bottom: 1px solid rgba(255, 255, 255, 0.07);',
      '  vertical-align: middle;',
      '}',
      'tbody tr:last-child td { border-bottom: 0; }',
      'tbody tr:hover td { background: rgba(228, 182, 94, 0.05); }',
      '.action-cell { font-weight: 600; color: #f5f2fa; }',
      '.detail-cell { color: #cfc6de; font-size: 12px; line-height: 1.4; overflow-wrap: anywhere; }',
      '.keys { display: inline-flex; flex-wrap: nowrap; align-items: center; gap: 3px; white-space: nowrap; }',
      'kbd {',
      '  display: inline-block;',
      '  padding: 2px 6px;',
      '  border-radius: 6px;',
      '  background: #120e18;',
      '  border: 1px solid rgba(228, 182, 94, 0.42);',
      '  box-shadow: 0 1px 0 rgba(228, 182, 94, 0.22);',
      '  color: #f0c674;',
      '  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;',
      '  font-size: 11px; font-weight: 600; line-height: 1.35;',
      '}',
      '.sep { color: #9c92ad; font-size: 11px; }',
      '.menu-only {',
      '  display: inline-block;',
      '  padding: 2px 7px;',
      '  border-radius: 6px;',
      '  background: rgba(255, 255, 255, 0.05);',
      '  border: 1px solid rgba(255, 255, 255, 0.14);',
      '  color: #cfc6de;',
      '  font-size: 11px; font-weight: 500;',
      '}',
      '.footer {',
      '  display: flex; align-items: center; justify-content: space-between; gap: 12px;',
      '  margin-top: 14px; flex-wrap: wrap;',
      '}',
      '.hint {',
      '  margin: 0;',
      '  font-size: 11.5px; line-height: 1.45; color: #b7aec7;',
      '}',
      '.hint strong { color: #f0c674; font-weight: 600; }',
      '.primary-btn {',
      '  padding: 8px 18px;',
      '  border-radius: 9px;',
      '  border: 1px solid #f0c674;',
      '  background: #e4b65e;',
      '  color: #120e18;',
      '  font-size: 13px; font-weight: 700;',
      '  cursor: pointer;',
      '  transition: background 140ms ease, transform 100ms ease;',
      '}',
      '.primary-btn:hover { background: #f0c674; }',
      '.primary-btn:active { transform: translateY(1px); }',
      '@media (max-width: 540px) {',
      '  .dialog { padding: 14px 14px 14px; }',
      '  h2 { font-size: 17px; }',
      '  .subtitle { font-size: 12px; }',
      '  .table-wrap { overflow-x: visible; }',
      '  thead { display: none; }',
      '  table, tbody { display: block; width: 100%; }',
      '  tbody tr {',
      '    display: grid;',
      '    grid-template-columns: minmax(0, 1fr) auto;',
      '    grid-template-areas: "action keys" "detail detail";',
      '    gap: 4px 10px;',
      '    padding: 9px 12px;',
      '    border-bottom: 1px solid rgba(255, 255, 255, 0.07);',
      '  }',
      '  tbody tr:last-child { border-bottom: 0; }',
      '  tbody tr:hover { background: rgba(228, 182, 94, 0.05); }',
      '  tbody tr:hover td { background: transparent; }',
      '  tbody td { display: block; padding: 0; border-bottom: 0; }',
      '  td.action-cell { grid-area: action; align-self: center; font-size: 12.5px; line-height: 1.3; }',
      '  td.keys-cell { grid-area: keys; align-self: center; justify-self: end; text-align: right; }',
      '  td.keys-cell .keys { flex-wrap: wrap; justify-content: flex-end; white-space: normal; }',
      '  td.detail-cell { grid-area: detail; font-size: 11.5px; line-height: 1.35; color: #b7aec7; }',
      '}'
    ].join('\n');

    var backdrop = document.createElement('div');
    backdrop.className = 'backdrop';

    var dialog = document.createElement('div');
    dialog.className = 'dialog';
    dialog.setAttribute('role', 'dialog');
    dialog.setAttribute('aria-modal', 'true');
    dialog.setAttribute('aria-labelledby', 'rongyok-shortcuts-title');

    var header = document.createElement('div');
    header.className = 'header';

    var titleWrap = document.createElement('div');
    titleWrap.className = 'title-wrap';

    var iconBadge = document.createElement('span');
    iconBadge.className = 'icon-badge';
    iconBadge.appendChild(createSvgIcon([
      ['rect', { x: '2', y: '5', width: '20', height: '14', rx: '2.5' }],
      ['path', { d: 'M6 10h.01M10 10h.01M14 10h.01M18 10h.01M6 14h.01M18 14h.01M9 14h6' }]
    ], 20));

    var textBlock = document.createElement('div');
    var eyebrow = document.createElement('p');
    eyebrow.className = 'eyebrow';
    eyebrow.textContent = 'Reference';

    var title = document.createElement('h2');
    title.id = 'rongyok-shortcuts-title';
    title.textContent = 'Keyboard Shortcuts';

    var subtitle = document.createElement('p');
    subtitle.className = 'subtitle';
    subtitle.textContent = 'Quick navigation and window controls for Rongyok Desk.';

    textBlock.appendChild(eyebrow);
    textBlock.appendChild(title);
    textBlock.appendChild(subtitle);

    titleWrap.appendChild(iconBadge);
    titleWrap.appendChild(textBlock);

    var closeBtn = document.createElement('button');
    closeBtn.type = 'button';
    closeBtn.className = 'close-btn';
    closeBtn.setAttribute('aria-label', 'Close');
    closeBtn.appendChild(createSvgIcon([
      ['path', { d: 'M18 6L6 18M6 6l12 12' }]
    ], 16));

    header.appendChild(titleWrap);
    header.appendChild(closeBtn);

    var tableWrap = document.createElement('div');
    tableWrap.className = 'table-wrap';

    var table = document.createElement('table');
    var thead = document.createElement('thead');
    var headRow = document.createElement('tr');
    var cols = ['Action', 'Shortcut', 'Details'];
    for (var c = 0; c < cols.length; c++) {
      var th = document.createElement('th');
      th.scope = 'col';
      th.textContent = cols[c];
      headRow.appendChild(th);
    }
    thead.appendChild(headRow);
    table.appendChild(thead);

    var tbody = document.createElement('tbody');
    var rows = [
      {
        action: 'Back',
        keys: [{ type: 'kbd', text: 'Alt' }, { type: 'sep', text: '+' }, { type: 'kbd', text: 'Left' }],
        detail: 'Also in Navigate menu'
      },
      {
        action: 'Forward',
        keys: [{ type: 'kbd', text: 'Alt' }, { type: 'sep', text: '+' }, { type: 'kbd', text: 'Right' }],
        detail: 'Also in Navigate menu'
      },
      {
        action: 'Reload',
        keys: [
          { type: 'kbd', text: 'F5' },
          { type: 'sep', text: '/' },
          { type: 'kbd', text: 'Ctrl+R' },
          { type: 'sep', text: '/' },
          { type: 'kbd', text: 'Cmd+R' }
        ],
        detail: 'Reloads page'
      },
      {
        action: 'Go to rongyok.com',
        keys: [{ type: 'kbd', text: 'Ctrl' }, { type: 'sep', text: '+' }, { type: 'kbd', text: 'H' }],
        detail: 'Opens https://rongyok.com/'
      },
      {
        action: 'Full screen',
        keys: [{ type: 'kbd', text: 'F11' }],
        detail: 'Toggles fullscreen'
      },
      {
        action: 'Open this page in browser',
        keys: [{ type: 'menu', text: 'Menu only' }],
        detail: 'Opens current page in system browser'
      },
      {
        action: 'Quit',
        keys: [{ type: 'kbd', text: 'Ctrl' }, { type: 'sep', text: '+' }, { type: 'kbd', text: 'Q' }],
        detail: 'Closes app'
      }
    ];

    for (var r = 0; r < rows.length; r++) {
      var tr = document.createElement('tr');
      var tdAction = document.createElement('td');
      tdAction.className = 'action-cell';
      tdAction.textContent = rows[r].action;

      var tdKeys = document.createElement('td');
      tdKeys.className = 'keys-cell';
      tdKeys.appendChild(buildKeyCombo(rows[r].keys));

      var tdDetail = document.createElement('td');
      tdDetail.className = 'detail-cell';
      tdDetail.textContent = rows[r].detail;

      tr.appendChild(tdAction);
      tr.appendChild(tdKeys);
      tr.appendChild(tdDetail);
      tbody.appendChild(tr);
    }
    table.appendChild(tbody);
    tableWrap.appendChild(table);

    var footer = document.createElement('div');
    footer.className = 'footer';

    var hint = document.createElement('p');
    hint.className = 'hint';
    hint.appendChild(document.createTextNode('Open anytime via '));
    var helpStrong = document.createElement('strong');
    helpStrong.textContent = 'Help > Keyboard Shortcuts';
    hint.appendChild(helpStrong);
    hint.appendChild(document.createTextNode(' (F1 or Ctrl+/). Press Esc to close.'));

    var primaryBtn = document.createElement('button');
    primaryBtn.type = 'button';
    primaryBtn.className = 'primary-btn';
    primaryBtn.textContent = 'Got it';

    footer.appendChild(hint);
    footer.appendChild(primaryBtn);

    dialog.appendChild(header);
    dialog.appendChild(tableWrap);
    dialog.appendChild(footer);
    backdrop.appendChild(dialog);

    shadow.appendChild(style);
    shadow.appendChild(backdrop);
    rootTarget.appendChild(host);

    backdrop.addEventListener('click', function (event) {
      if (event.target === backdrop) {
        closeModal();
      }
    });
    closeBtn.addEventListener('click', closeModal);
    primaryBtn.addEventListener('click', closeModal);

    ui = {
      host: host,
      eyebrow: eyebrow,
      title: title,
      primaryBtn: primaryBtn
    };
    return ui;
  }

  function isModalOpen() {
    return modalOpen;
  }

  function openModal(isOnboarding) {
    if (window.__rongyokCloseAbout) {
      try { window.__rongyokCloseAbout(); } catch (_) {}
    }
    if (isOnboarding) {
      onboardingActive = true;
    }
    var parts = ensureUi();
    if (!parts) {
      if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', function () {
          openModal(isOnboarding);
        }, { once: true });
      }
      return;
    }

    if (onboardingActive) {
      parts.eyebrow.textContent = 'Welcome to Rongyok Desk';
      parts.title.textContent = 'Keyboard Shortcuts';
    } else {
      parts.eyebrow.textContent = 'Help Reference';
      parts.title.textContent = 'Keyboard Shortcuts';
    }

    if (!modalOpen) {
      lastFocused = document.activeElement;
      modalOpen = true;
      parts.host.setAttribute('data-rongyok-open', 'true');
      requestAnimationFrame(function () {
        try { parts.primaryBtn.focus(); } catch (_) {}
      });
    }
  }

  function closeModal() {
    if (!modalOpen) return;
    modalOpen = false;
    if (ui) {
      ui.host.removeAttribute('data-rongyok-open');
    }
    if (lastFocused && lastFocused.isConnected && typeof lastFocused.focus === 'function') {
      try { lastFocused.focus(); } catch (_) {}
    }
    lastFocused = null;

    if (onboardingActive) {
      onboardingActive = false;
      window.location.href = DISMISS_URL;
    }
  }

  window.__rongyokShowShortcuts = openModal;
  window.__rongyokCloseShortcuts = closeModal;

  window.addEventListener('keydown', function (event) {
    var key = (event.key || '').toLowerCase();
    var alt = event.altKey;
    var ctrl = event.ctrlKey || event.metaKey;

    if (key === 'escape' && isModalOpen()) {
      event.preventDefault();
      event.stopPropagation();
      event.stopImmediatePropagation();
      closeModal();
      return;
    }
    if (!alt && !ctrl && !event.shiftKey && key === 'f1') {
      event.preventDefault();
      event.stopPropagation();
      event.stopImmediatePropagation();
      openModal(false);
      return;
    }
    if (!alt && ctrl && !event.shiftKey && (key === '/' || event.code === 'Slash')) {
      event.preventDefault();
      event.stopPropagation();
      event.stopImmediatePropagation();
      if (isModalOpen()) {
        closeModal();
      } else {
        openModal(false);
      }
      return;
    }
    if (alt && !ctrl && (key === 'arrowleft' || key === 'left')) {
      event.preventDefault();
      history.back();
    } else if (alt && !ctrl && (key === 'arrowright' || key === 'right')) {
      event.preventDefault();
      history.forward();
    } else if ((!alt && !ctrl && key === 'f5') || (!alt && ctrl && key === 'r')) {
      event.preventDefault();
      location.reload();
    }
  }, true);
})();
"#;

/// Escape a string for safe inclusion inside a single-quoted JavaScript literal.
fn escape_js_single_quoted(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

/// Initialization script for the isolated Shadow DOM "About Rongyok Desk" modal.
/// Interpolates authoritative compile-time metadata from Cargo so no stale copy exists.
fn about_script() -> String {
    let app_name = escape_js_single_quoted(APP_NAME);
    let version_badge = escape_js_single_quoted(&format!("v{APP_VERSION}"));
    let platform = escape_js_single_quoted(&format!(
        "{} / {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    let version_info = escape_js_single_quoted(&version_info_string());
    let description = escape_js_single_quoted(APP_DESCRIPTION);
    let site_url = escape_js_single_quoted(START_URL);

    let template = r#"
(function () {
  if (window.__rongyokAboutInit) return;
  window.__rongyokAboutInit = true;

  var APP_NAME = '__APP_NAME__';
  var VERSION_BADGE = '__VERSION_BADGE__';
  var PLATFORM = '__PLATFORM__';
  var VERSION_INFO = '__VERSION_INFO__';
  var DESCRIPTION = '__DESCRIPTION__';
  var SITE_URL = '__SITE_URL__';
  var COPYRIGHT = '__COPYRIGHT__';
  var HOST_ID = 'rongyok-desk-about-host';

  var modalOpen = false;
  var lastFocused = null;
  var copyResetTimer = null;
  var ui = null;
  var SVG_NS = 'http://www.w3.org/2000/svg';

  function createSvgIcon(paths, size) {
    var svg = document.createElementNS(SVG_NS, 'svg');
    svg.setAttribute('viewBox', '0 0 24 24');
    svg.setAttribute('width', String(size));
    svg.setAttribute('height', String(size));
    svg.setAttribute('fill', 'none');
    svg.setAttribute('stroke', 'currentColor');
    svg.setAttribute('stroke-width', '1.85');
    svg.setAttribute('stroke-linecap', 'round');
    svg.setAttribute('stroke-linejoin', 'round');
    svg.setAttribute('aria-hidden', 'true');
    for (var i = 0; i < paths.length; i++) {
      var el = document.createElementNS(SVG_NS, paths[i][0]);
      var attrs = paths[i][1];
      for (var k in attrs) {
        if (Object.prototype.hasOwnProperty.call(attrs, k)) {
          el.setAttribute(k, attrs[k]);
        }
      }
      svg.appendChild(el);
    }
    return svg;
  }

  function addMetaRow(tbody, label, valueNode) {
    var tr = document.createElement('tr');
    var th = document.createElement('th');
    th.scope = 'row';
    th.textContent = label;
    var td = document.createElement('td');
    td.appendChild(valueNode);
    tr.appendChild(th);
    tr.appendChild(td);
    tbody.appendChild(tr);
  }

  function ensureUi() {
    if (ui) {
      var rootTarget = document.documentElement || document.body;
      if (rootTarget && !ui.host.isConnected) {
        rootTarget.appendChild(ui.host);
      }
      return ui;
    }

    var rootTarget = document.documentElement || document.body;
    if (!rootTarget) return null;

    var host = document.createElement('div');
    host.id = HOST_ID;

    var shadow = host.attachShadow({ mode: 'closed' });

    var style = document.createElement('style');
    style.textContent = [
      ':host {',
      '  all: initial !important;',
      '  position: fixed !important;',
      '  inset: 0 !important;',
      '  width: 100vw !important;',
      '  height: 100vh !important;',
      '  margin: 0 !important;',
      '  padding: 0 !important;',
      '  border: 0 !important;',
      '  background: transparent !important;',
      '  z-index: 2147483647 !important;',
      '  isolation: isolate !important;',
      '  display: none !important;',
      '  pointer-events: none !important;',
      '  font-family: "Segoe UI", system-ui, -apple-system, sans-serif !important;',
      '  color: #f5f2fa !important;',
      '}',
      ':host([data-rongyok-open="true"]) {',
      '  display: block !important;',
      '  pointer-events: auto !important;',
      '}',
      '*, *::before, *::after { box-sizing: border-box; }',
      'svg { pointer-events: none; }',
      '.backdrop {',
      '  position: fixed; inset: 0;',
      '  display: flex; align-items: center; justify-content: center;',
      '  padding: 12px;',
      '  background: rgba(12, 9, 18, 0.78);',
      '  backdrop-filter: blur(6px);',
      '  -webkit-backdrop-filter: blur(6px);',
      '}',
      '.dialog {',
      '  width: min(460px, calc(100vw - 24px));',
      '  max-height: calc(100vh - 24px);',
      '  overflow-y: auto;',
      '  border-radius: 16px;',
      '  background: linear-gradient(180deg, #1b1526 0%, #120e18 100%);',
      '  border: 1px solid rgba(228, 182, 94, 0.36);',
      '  box-shadow: 0 24px 64px rgba(0, 0, 0, 0.68), 0 0 0 1px rgba(255, 255, 255, 0.04) inset;',
      '  color: #f5f2fa;',
      '  padding: 18px 20px;',
      '}',
      '.header {',
      '  display: flex; align-items: flex-start; justify-content: space-between; gap: 12px;',
      '  margin-bottom: 12px;',
      '}',
      '.title-wrap { display: flex; align-items: flex-start; gap: 12px; min-width: 0; }',
      '.icon-badge {',
      '  flex: 0 0 auto;',
      '  width: 38px; height: 38px;',
      '  border-radius: 10px;',
      '  display: inline-flex; align-items: center; justify-content: center;',
      '  background: rgba(228, 182, 94, 0.14);',
      '  border: 1px solid rgba(228, 182, 94, 0.38);',
      '  color: #e4b65e;',
      '}',
      '.eyebrow {',
      '  margin: 0 0 2px;',
      '  font-size: 10.5px; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase;',
      '  color: #e4b65e;',
      '}',
      'h2 {',
      '  margin: 0;',
      '  font-size: 18px; font-weight: 700; line-height: 1.25; color: #f5f2fa;',
      '}',
      '.version-pill {',
      '  display: inline-block; margin-left: 8px; padding: 2px 8px;',
      '  border-radius: 999px; vertical-align: middle;',
      '  background: rgba(228, 182, 94, 0.14);',
      '  border: 1px solid rgba(228, 182, 94, 0.4);',
      '  color: #f0c674;',
      '  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;',
      '  font-size: 11.5px; font-weight: 600;',
      '}',
      '.desc {',
      '  margin: 0 0 14px;',
      '  font-size: 13px; line-height: 1.5; color: #cfc6de;',
      '}',
      '.close-btn {',
      '  flex: 0 0 auto;',
      '  width: 32px; height: 32px;',
      '  border-radius: 8px;',
      '  border: 1px solid rgba(255, 255, 255, 0.12);',
      '  background: rgba(255, 255, 255, 0.04);',
      '  color: #cfc6de;',
      '  display: inline-flex; align-items: center; justify-content: center;',
      '  cursor: pointer;',
      '  transition: background 140ms ease, border-color 140ms ease, color 140ms ease;',
      '}',
      '.close-btn:hover { background: rgba(228, 182, 94, 0.14); border-color: rgba(228, 182, 94, 0.4); color: #f5f2fa; }',
      '.close-btn:focus-visible, .secondary-btn:focus-visible, .primary-btn:focus-visible, .site-link:focus-visible {',
      '  outline: 2px solid #f0c674; outline-offset: 2px;',
      '}',
      '.meta-wrap {',
      '  border-radius: 12px;',
      '  border: 1px solid rgba(255, 255, 255, 0.1);',
      '  background: rgba(18, 14, 24, 0.72);',
      '  overflow: hidden;',
      '}',
      'table { width: 100%; border-collapse: collapse; font-size: 12.5px; }',
      'th {',
      '  width: 118px; text-align: left; font-weight: 600; color: #b7aec7;',
      '  padding: 9px 12px; border-bottom: 1px solid rgba(255, 255, 255, 0.07);',
      '  vertical-align: middle;',
      '}',
      'td {',
      '  padding: 9px 12px; color: #f5f2fa;',
      '  border-bottom: 1px solid rgba(255, 255, 255, 0.07);',
      '  vertical-align: middle; overflow-wrap: anywhere;',
      '}',
      'tr:last-child th, tr:last-child td { border-bottom: 0; }',
      '.mono {',
      '  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;',
      '  font-size: 12px; color: #f0c674;',
      '}',
      '.site-link { color: #f0c674; text-decoration: none; font-weight: 600; }',
      '.site-link:hover { text-decoration: underline; }',
      '.footer {',
      '  display: flex; align-items: center; justify-content: space-between; gap: 10px;',
      '  margin-top: 16px; flex-wrap: wrap;',
      '}',
      '.secondary-btn {',
      '  display: inline-flex; align-items: center; gap: 6px;',
      '  padding: 8px 13px;',
      '  border-radius: 9px;',
      '  border: 1px solid rgba(228, 182, 94, 0.36);',
      '  background: rgba(228, 182, 94, 0.1);',
      '  color: #f0c674;',
      '  font-size: 12.5px; font-weight: 600;',
      '  cursor: pointer;',
      '  transition: background 140ms ease, border-color 140ms ease;',
      '}',
      '.secondary-btn:hover { background: rgba(228, 182, 94, 0.18); border-color: rgba(228, 182, 94, 0.55); }',
      '.secondary-btn[data-copied="true"] {',
      '  background: rgba(228, 182, 94, 0.24); border-color: #f0c674; color: #f5f2fa;',
      '}',
      '.primary-btn {',
      '  padding: 8px 18px;',
      '  border-radius: 9px;',
      '  border: 1px solid #f0c674;',
      '  background: #e4b65e;',
      '  color: #120e18;',
      '  font-size: 13px; font-weight: 700;',
      '  cursor: pointer;',
      '  transition: background 140ms ease, transform 100ms ease;',
      '}',
      '.primary-btn:hover { background: #f0c674; }',
      '.primary-btn:active { transform: translateY(1px); }',
      '@media (max-width: 440px) {',
      '  .dialog { padding: 15px 14px; }',
      '  th { width: 92px; padding: 8px 10px; font-size: 12px; }',
      '  td { padding: 8px 10px; font-size: 12px; }',
      '  .footer { flex-direction: column; align-items: stretch; }',
      '  .secondary-btn, .primary-btn { justify-content: center; width: 100%; }',
      '}'
    ].join('\n');

    var backdrop = document.createElement('div');
    backdrop.className = 'backdrop';

    var dialog = document.createElement('div');
    dialog.className = 'dialog';
    dialog.setAttribute('role', 'dialog');
    dialog.setAttribute('aria-modal', 'true');
    dialog.setAttribute('aria-labelledby', 'rongyok-about-title');

    var header = document.createElement('div');
    header.className = 'header';

    var titleWrap = document.createElement('div');
    titleWrap.className = 'title-wrap';

    var iconBadge = document.createElement('span');
    iconBadge.className = 'icon-badge';
    iconBadge.appendChild(createSvgIcon([
      ['circle', { cx: '12', cy: '12', r: '9' }],
      ['path', { d: 'M12 16v-4M12 8h.01' }]
    ], 20));

    var textBlock = document.createElement('div');
    var eyebrow = document.createElement('p');
    eyebrow.className = 'eyebrow';
    eyebrow.textContent = 'About';

    var title = document.createElement('h2');
    title.id = 'rongyok-about-title';
    title.appendChild(document.createTextNode(APP_NAME));
    var versionPill = document.createElement('span');
    versionPill.className = 'version-pill';
    versionPill.textContent = VERSION_BADGE;
    title.appendChild(versionPill);

    textBlock.appendChild(eyebrow);
    textBlock.appendChild(title);
    titleWrap.appendChild(iconBadge);
    titleWrap.appendChild(textBlock);

    var closeXBtn = document.createElement('button');
    closeXBtn.type = 'button';
    closeXBtn.className = 'close-btn';
    closeXBtn.setAttribute('aria-label', 'Close');
    closeXBtn.appendChild(createSvgIcon([
      ['path', { d: 'M18 6L6 18M6 6l12 12' }]
    ], 16));

    header.appendChild(titleWrap);
    header.appendChild(closeXBtn);

    var desc = document.createElement('p');
    desc.className = 'desc';
    desc.textContent = DESCRIPTION;

    var metaWrap = document.createElement('div');
    metaWrap.className = 'meta-wrap';
    var table = document.createElement('table');
    var tbody = document.createElement('tbody');

    var verSpan = document.createElement('span');
    verSpan.className = 'mono';
    verSpan.textContent = VERSION_BADGE + ' (' + PLATFORM + ')';
    addMetaRow(tbody, 'Version', verSpan);

    var siteLink = document.createElement('a');
    siteLink.className = 'site-link';
    siteLink.href = SITE_URL;
    siteLink.textContent = SITE_URL;
    siteLink.addEventListener('click', function () {
      closeModal();
    });
    addMetaRow(tbody, 'Website', siteLink);

    var copyrightNode = document.createTextNode(COPYRIGHT);
    addMetaRow(tbody, 'Copyright', copyrightNode);

    table.appendChild(tbody);
    metaWrap.appendChild(table);

    var footer = document.createElement('div');
    footer.className = 'footer';

    var copyBtn = document.createElement('button');
    copyBtn.type = 'button';
    copyBtn.className = 'secondary-btn';
    var copyIcon = createSvgIcon([
      ['rect', { x: '9', y: '9', width: '11', height: '11', rx: '2' }],
      ['path', { d: 'M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1' }]
    ], 14);
    var copyLabel = document.createElement('span');
    copyLabel.textContent = 'Copy Version Info';
    copyBtn.appendChild(copyIcon);
    copyBtn.appendChild(copyLabel);

    var closeBtn = document.createElement('button');
    closeBtn.type = 'button';
    closeBtn.className = 'primary-btn';
    closeBtn.textContent = 'Close';

    footer.appendChild(copyBtn);
    footer.appendChild(closeBtn);

    dialog.appendChild(header);
    dialog.appendChild(desc);
    dialog.appendChild(metaWrap);
    dialog.appendChild(footer);
    backdrop.appendChild(dialog);

    shadow.appendChild(style);
    shadow.appendChild(backdrop);
    rootTarget.appendChild(host);

    function showCopiedFeedback() {
      copyLabel.textContent = 'Copied!';
      copyBtn.setAttribute('data-copied', 'true');
      if (copyResetTimer) {
        clearTimeout(copyResetTimer);
      }
      copyResetTimer = setTimeout(function () {
        copyResetTimer = null;
        copyLabel.textContent = 'Copy Version Info';
        copyBtn.removeAttribute('data-copied');
      }, 2000);
    }

    function fallbackCopy(text) {
      var ta = document.createElement('textarea');
      ta.value = text;
      ta.setAttribute('readonly', '');
      ta.style.position = 'fixed';
      ta.style.opacity = '0';
      ta.style.pointerEvents = 'none';
      shadow.appendChild(ta);
      ta.select();
      try {
        if (document.execCommand('copy')) {
          showCopiedFeedback();
        }
      } catch (_) {}
      shadow.removeChild(ta);
      try { copyBtn.focus(); } catch (_) {}
    }

    copyBtn.addEventListener('click', function () {
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(VERSION_INFO).then(showCopiedFeedback, function () {
          fallbackCopy(VERSION_INFO);
        });
      } else {
        fallbackCopy(VERSION_INFO);
      }
    });

    backdrop.addEventListener('click', function (event) {
      if (event.target === backdrop) {
        closeModal();
      }
    });
    closeXBtn.addEventListener('click', closeModal);
    closeBtn.addEventListener('click', closeModal);

    ui = {
      host: host,
      closeBtn: closeBtn,
      copyBtn: copyBtn,
      copyLabel: copyLabel
    };
    return ui;
  }

  function isModalOpen() {
    return modalOpen;
  }

  function openModal() {
    if (window.__rongyokCloseShortcuts) {
      try { window.__rongyokCloseShortcuts(); } catch (_) {}
    }
    var parts = ensureUi();
    if (!parts) {
      if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', openModal, { once: true });
      }
      return;
    }
    if (!modalOpen) {
      lastFocused = document.activeElement;
      modalOpen = true;
      parts.host.setAttribute('data-rongyok-open', 'true');
      requestAnimationFrame(function () {
        try { parts.closeBtn.focus(); } catch (_) {}
      });
    }
  }

  function closeModal() {
    if (!modalOpen) return;
    modalOpen = false;
    if (copyResetTimer) {
      clearTimeout(copyResetTimer);
      copyResetTimer = null;
    }
    if (ui) {
      ui.copyLabel.textContent = 'Copy Version Info';
      ui.copyBtn.removeAttribute('data-copied');
      ui.host.removeAttribute('data-rongyok-open');
    }
    if (lastFocused && lastFocused.isConnected && typeof lastFocused.focus === 'function') {
      try { lastFocused.focus(); } catch (_) {}
    }
    lastFocused = null;
  }

  window.__rongyokShowAbout = openModal;
  window.__rongyokCloseAbout = closeModal;

  window.addEventListener('keydown', function (event) {
    var key = (event.key || '').toLowerCase();
    if (key === 'escape' && isModalOpen()) {
      event.preventDefault();
      event.stopPropagation();
      event.stopImmediatePropagation();
      closeModal();
    }
  }, true);
})();
"#;

    template
        .replace("__APP_NAME__", &app_name)
        .replace("__VERSION_BADGE__", &version_badge)
        .replace("__PLATFORM__", &platform)
        .replace("__VERSION_INFO__", &version_info)
        .replace("__DESCRIPTION__", &description)
        .replace("__SITE_URL__", &site_url)
        .replace("__COPYRIGHT__", APP_COPYRIGHT_JS)
}
#[cfg(target_os = "windows")]
unsafe fn read_png_stream(stream: &windows::Win32::System::Com::IStream) -> Option<Vec<u8>> {
    use windows::Win32::System::Com::{STATFLAG_NONAME, STATSTG};
    use windows::Win32::System::Com::StructuredStorage::GetHGlobalFromStream;
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

    let mut stats = STATSTG::default();
    stream.Stat(&mut stats, STATFLAG_NONAME).ok()?;
    let written = stats.cbSize as usize;
    if written == 0 {
        return None;
    }

    let handle = GetHGlobalFromStream(stream).ok()?;
    let start = GlobalLock(handle) as *const u8;
    if start.is_null() {
        return None;
    }
    let available = GlobalSize(handle);
    let take = written.min(available);
    let bytes = std::slice::from_raw_parts(start, take).to_vec();
    let _ = GlobalUnlock(handle);
    Some(bytes)
}

/// Ask the embedded webview for a picture of what it is showing, as PNG bytes.
#[cfg(target_os = "windows")]
fn grab_window_png(window: &WebviewWindow) -> Option<Vec<u8>> {
    use std::sync::{Arc, Mutex};

    use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG;
    use webview2_com::CapturePreviewCompletedHandler;
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::Com::StructuredStorage::CreateStreamOnHGlobal;

    // None means "not finished yet"; Some(picture) is the answer, itself possibly None.
    type Slot = Arc<Mutex<Option<Option<Vec<u8>>>>>;
    let result: Slot = Arc::new(Mutex::new(None));
    let result_for_webview = Arc::clone(&result);

    let dispatched = window.with_webview(move |platform| {
        unsafe {
            let controller = platform.controller();
            let webview = match controller.CoreWebView2() {
                Ok(webview) => webview,
                Err(_) => {
                    if let Ok(mut slot) = result_for_webview.lock() {
                        *slot = Some(None);
                    }
                    return;
                }
            };

            let stream = match CreateStreamOnHGlobal(HGLOBAL(std::ptr::null_mut()), true) {
                Ok(stream) => stream,
                Err(_) => {
                    if let Ok(mut slot) = result_for_webview.lock() {
                        *slot = Some(None);
                    }
                    return;
                }
            };

            let stream_for_callback = stream.clone();
            let result_for_callback = Arc::clone(&result_for_webview);
            let handler =
                CapturePreviewCompletedHandler::create(Box::new(move |status: windows::core::Result<()>| {
                    let picture = if status.is_ok() {
                        read_png_stream(&stream_for_callback)
                    } else {
                        None
                    };
                    if let Ok(mut slot) = result_for_callback.lock() {
                        *slot = Some(picture);
                    }
                    Ok(())
                }));

            if webview
                .CapturePreview(
                    COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
                    &stream,
                    &handler,
                )
                .is_err()
            {
                if let Ok(mut slot) = result_for_webview.lock() {
                    *slot = Some(None);
                }
            }
        }
    });

    if dispatched.is_err() {
        return None;
    }

    // The picture arrives on the window's own thread; wait for it here.
    let deadline = std::time::Instant::now() + Duration::from_secs(25);
    loop {
        if let Ok(slot) = result.lock() {
            if let Some(finished) = slot.as_ref() {
                return finished.clone();
            }
        }
        if std::time::Instant::now() > deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(40));
    }
}

#[cfg(not(target_os = "windows"))]
fn grab_window_png(_window: &WebviewWindow) -> Option<Vec<u8>> {
    // Only the Windows webview offers a capture call that works while the window
    // is covered; elsewhere the check cannot take a picture.
    None
}

/// Wait for the window to draw, then save a picture of it and quit.
fn run_screenshot(app: AppHandle, path: PathBuf) {
    std::thread::spawn(move || {
        // About 120 frames at 60 a second: enough for the page to paint once.
        for _ in 0..120 {
            std::thread::sleep(Duration::from_millis(16));
        }
        std::thread::sleep(Duration::from_millis(400));

        let mut last_picture: Option<Vec<u8>> = None;
        let mut saved = false;

        // A page that is still loading can photograph blank, so try again for a while.
        for _ in 0..30 {
            let picture = app
                .get_webview_window(WINDOW_LABEL)
                .and_then(|window| grab_window_png(&window));

            if let Some(bytes) = picture {
                let blank = png_looks_blank(&bytes);
                last_picture = Some(bytes);
                if !blank {
                    if let Some(parent) = path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if std::fs::write(&path, last_picture.as_ref().unwrap()).is_ok() {
                        saved = true;
                        break;
                    }
                }
            }

            std::thread::sleep(Duration::from_millis(600));
        }

        // Even a blank picture is better than none: it tells the person what happened.
        if !saved {
            if let Some(bytes) = last_picture {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::write(&path, bytes);
            }
        }

        app.exit(if saved { 0 } else { 1 });
    });
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let back = MenuItem::with_id(app, "back", "Back", true, Some("Alt+Left"))?;
    let forward = MenuItem::with_id(app, "forward", "Forward", true, Some("Alt+Right"))?;
    let reload = MenuItem::with_id(app, "reload", "Reload", true, Some("F5"))?;
    let home = MenuItem::with_id(app, "home", "Go to rongyok.com", true, Some("Ctrl+H"))?;
    let fullscreen = MenuItem::with_id(app, "fullscreen", "Full screen", true, Some("F11"))?;
    let in_browser = MenuItem::with_id(app, "browser", "Open this page in browser", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, Some("Ctrl+Q"))?;

    let separator_one = PredefinedMenuItem::separator(app)?;
    let separator_two = PredefinedMenuItem::separator(app)?;
    let separator_three = PredefinedMenuItem::separator(app)?;

    let navigate = Submenu::with_items(
        app,
        "Navigate",
        true,
        &[
            &back,
            &forward,
            &reload,
            &home,
            &separator_one,
            &fullscreen,
            &separator_two,
            &in_browser,
            &separator_three,
            &quit,
        ],
    )?;

    let shortcuts = MenuItem::with_id(
        app,
        "shortcuts",
        "Keyboard Shortcuts",
        true,
        Some("F1"),
    )?;
    let help_separator = PredefinedMenuItem::separator(app)?;
    let about = MenuItem::with_id(
        app,
        "about",
        "About Rongyok Desk",
        true,
        None::<&str>,
    )?;
    let help = Submenu::with_items(app, "Help", true, &[&shortcuts, &help_separator, &about])?;

    Menu::with_items(app, &[&navigate, &help])
}

fn main() {
    // Read --screenshot <file.png> (or --screenshot=<file.png>) from the command line.
    let arguments: Vec<String> = std::env::args().collect();
    let mut screenshot_path: Option<PathBuf> = None;
    let mut index = 1;
    while index < arguments.len() {
        let argument = &arguments[index];
        if argument == "--screenshot" {
            if index + 1 < arguments.len() {
                screenshot_path = Some(PathBuf::from(&arguments[index + 1]));
                index += 2;
                continue;
            }
        } else if let Some(value) = argument.strip_prefix("--screenshot=") {
            screenshot_path = Some(PathBuf::from(value));
        }
        index += 1;
    }
    let taking_picture = screenshot_path.is_some();

    let app = tauri::Builder::default()
        .on_menu_event(|app, event| {
            let Some(window) = app.get_webview_window(WINDOW_LABEL) else {
                return;
            };
            match event.id().as_ref() {
                "back" => {
                    let _ = window.eval("history.back()");
                }
                "forward" => {
                    let _ = window.eval("history.forward()");
                }
                "reload" => {
                    let _ = window.eval("location.reload()");
                }
                "home" => {
                    if let Ok(url) = Url::parse(START_URL) {
                        let _ = window.navigate(url);
                    }
                }
                "fullscreen" => {
                    let now = window.is_fullscreen().unwrap_or(false);
                    let _ = window.set_fullscreen(!now);
                }
                "browser" => {
                    if let Ok(url) = window.url() {
                        open_in_system_browser(url.as_str());
                    }
                }
                "shortcuts" => {
                    let _ = window.eval(
                        "window.__rongyokShowShortcuts && window.__rongyokShowShortcuts(false);",
                    );
                }
                "about" => {
                    let _ = window.eval(
                        "window.__rongyokShowAbout && window.__rongyokShowAbout();",
                    );
                }
                "quit" => app.exit(0),
                _ => {}
            }
        })
        .setup(move |app| {
            let handle = app.handle().clone();
            let onboarding_pending = Arc::new(AtomicBool::new(should_show_onboarding(
                taking_picture,
                has_seen_onboarding(&handle),
            )));

            // Keep cookies and site storage in a steady folder, so the person stays
            // signed in between launches.
            let data_folder = handle
                .path()
                .app_local_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir())
                .join("webview");

            let mut window_options = WebviewWindowBuilder::new(
                app,
                WINDOW_LABEL,
                WebviewUrl::External(Url::parse(START_URL).unwrap()),
            )
            .title("Rongyok Desk")
            .inner_size(1280.0, 800.0)
            .min_inner_size(520.0, 400.0)
            .data_directory(data_folder)
            // Keep drawing even when the window is not in front, or a picture taken
            // by the check would come out empty.
            .background_throttling(BackgroundThrottlingPolicy::Disabled)
            // Video and audio start without the person having to click first.
            .additional_browser_args(
                "--autoplay-policy=no-user-gesture-required \
                 --disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection",
            )
            // The page inspector is only available while developing.
            .devtools(cfg!(debug_assertions))
            // The keys a desktop window keeps working even if the page swallows them.
            .initialization_script(SHORTCUT_SCRIPT)
            .initialization_script(about_script())
            .on_navigation({
                let handle = handle.clone();
                let onboarding_pending = Arc::clone(&onboarding_pending);
                move |url| {
                    if is_onboarding_dismiss_url(url) {
                        onboarding_pending.store(false, Ordering::SeqCst);
                        mark_onboarding_seen(&handle);
                        return false;
                    }
                    if is_internal(url) {
                        return true;
                    }
                    // A link leaving the site: hand it to the normal browser, keep this
                    // window where it is. mailto:, tel: and the like go there too.
                    open_in_system_browser(url.as_str());
                    false
                }
            })
            .on_new_window({
                let handle = handle.clone();
                let onboarding_pending = Arc::clone(&onboarding_pending);
                move |url, _features| {
                    if is_onboarding_dismiss_url(&url) {
                        onboarding_pending.store(false, Ordering::SeqCst);
                        mark_onboarding_seen(&handle);
                        return tauri::webview::NewWindowResponse::Deny;
                    }
                    if is_internal(&url) {
                        // A same-site pop-up stays in this window rather than opening
                        // a second one.
                        if let Some(window) = handle.get_webview_window(WINDOW_LABEL) {
                            let _ = window.navigate(url);
                        }
                    } else {
                        open_in_system_browser(url.as_str());
                    }
                    tauri::webview::NewWindowResponse::Deny
                }
            })
            .on_page_load({
                let onboarding_pending = Arc::clone(&onboarding_pending);
                move |window, payload| {
                    if should_auto_open_on_page_load(
                        payload.event(),
                        payload.url(),
                        onboarding_pending.load(Ordering::Acquire),
                    ) {
                        let _ = window.eval(
                            "window.__rongyokShowShortcuts && window.__rongyokShowShortcuts(true);",
                        );
                    }
                }
            });

            if taking_picture {
                // Open without stealing focus, and on Windows behind other windows,
                // so a check never interrupts what the person is doing.
                window_options = window_options.focused(false).always_on_bottom(true);
            }

            // Reopen where the window was left, so it feels like an app and not a
            // fresh tab every time. A picture is always taken at the standard size,
            // and a placement from a monitor that is no longer attached is ignored.
            let restored = if taking_picture {
                None
            } else {
                load_placement(&handle).filter(|p| position_is_on_a_screen(&handle, p))
            };
            if let Some(placement) = restored {
                // Only the spot is set here. The size is applied below, once the menu
                // bar exists, so the saved outer size is reproduced exactly whatever
                // the frame and menu happen to add up to.
                window_options = window_options
                    .position(placement.x as f64, placement.y as f64)
                    .maximized(placement.maximized);
            }

            let window = window_options.build()?;

            // Remember where the window was, but not while taking a picture.
            if !taking_picture {
                remember_placement(&handle, &window);
            }

            if let Ok(menu) = build_menu(&handle) {
                let _ = window.set_menu(menu);
            }

            if let Some(placement) = restored {
                if !placement.maximized {
                    // The size call works on the area inside the frame and menu, while
                    // the saved note is the whole window. Measure what the frame and
                    // menu add on this machine, subtract it, and the window comes back
                    // at exactly the size it was left.
                    let frame_extra = match (window.outer_size(), window.inner_size()) {
                        (Ok(outer), Ok(inner)) => Some((
                            outer.width.saturating_sub(inner.width),
                            outer.height.saturating_sub(inner.height),
                        )),
                        _ => None,
                    };
                    if let Some((extra_width, extra_height)) = frame_extra {
                        let width = placement.width.saturating_sub(extra_width).max(520);
                        let height = placement.height.saturating_sub(extra_height).max(400);
                        let _ =
                            window.set_size(tauri::PhysicalSize::new(width, height));
                    }
                }
            }

            // A video going fullscreen should fill the screen, not just the page.
            watch_video_fullscreen(&window);

            if let Some(path) = screenshot_path.clone() {
                run_screenshot(handle, path);
            } else {
                let _ = window.set_focus();
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build the Rongyok Desk window");

    app.run(|_app, _event| {});
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocked(url: &str) -> bool {
        !is_internal(&Url::parse(url).unwrap())
    }

    #[test]
    fn the_site_itself_is_allowed() {
        assert!(is_internal(&Url::parse("https://rongyok.com/").unwrap()));
        assert!(is_internal(
            &Url::parse("https://rongyok.com/series/100628695/").unwrap()
        ));
        assert!(is_internal(&Url::parse("http://rongyok.com/").unwrap()));
    }

    #[test]
    fn subdomains_are_allowed() {
        assert!(is_internal(&Url::parse("https://www.rongyok.com/").unwrap()));
        assert!(is_internal(
            &Url::parse("https://cdn.deep.rongyok.com/a.mp4").unwrap()
        ));
    }

    #[test]
    fn other_sites_are_blocked() {
        assert!(blocked("https://example.com/"));
        assert!(blocked("https://google.com/search?q=rongyok"));
        assert!(blocked("http://facebook.com/rongyok"));
        assert!(blocked("https://notrongyok.com/"));
        assert!(blocked("https://rongyok.com.evil.example/"));
        assert!(blocked("https://evil.example/rongyok.com"));
        assert!(blocked("https://rongyokxcom.example/"));
    }

    #[test]
    fn lookalike_hosts_do_not_slip_through() {
        // A host that merely ends with the name but is a different domain.
        assert!(blocked("https://fakerongyok.com/"));
        assert!(blocked("https://rongyok.com@evil.example/"));
    }

    #[test]
    fn mail_and_app_links_are_foreign() {
        assert!(blocked("mailto:someone@example.com"));
        assert!(blocked("tel:+661234567"));
    }

    #[test]
    fn the_webview_own_pages_are_allowed() {
        assert!(is_internal(&Url::parse("about:blank").unwrap()));
        assert!(is_internal(&Url::parse("data:text/html,hi").unwrap()));
        assert!(is_internal(&Url::parse("blob:https://rongyok.com/abc").unwrap()));
    }

    #[test]
    fn a_saved_placement_round_trips() {
        let placement = WindowPlacement {
            x: -1280,
            y: 40,
            width: 1024,
            height: 720,
            maximized: false,
        };
        let text = format_placement(&placement);
        assert_eq!(text, "-1280 40 1024 720 0");
        assert_eq!(parse_placement(&text), Some(placement));
    }

    #[test]
    fn a_maximized_placement_is_remembered() {
        let placement = WindowPlacement {
            x: 100,
            y: 100,
            width: 1600,
            height: 900,
            maximized: true,
        };
        assert_eq!(
            parse_placement(&format_placement(&placement)),
            Some(placement)
        );
    }

    #[test]
    fn a_damaged_placement_file_is_ignored() {
        // Anything unreadable must fall back to the default window, never panic.
        assert_eq!(parse_placement(""), None);
        assert_eq!(parse_placement("garbage"), None);
        assert_eq!(parse_placement("1 2 3"), None);
        assert_eq!(parse_placement("1 2 3 4 5 6"), None);
        assert_eq!(parse_placement("a b c d e"), None);
        // A window too small to use, or a silly maximized flag, is refused.
        assert_eq!(parse_placement("0 0 10 10 0"), None);
        assert_eq!(parse_placement("0 0 800 600 maybe"), None);
    }

    #[test]
    fn an_oversized_placement_is_refused() {
        let silly = WindowPlacement {
            x: 0,
            y: 0,
            width: 100_000,
            height: 600,
            maximized: false,
        };
        assert!(!silly.is_usable());
        assert_eq!(parse_placement(&format_placement(&silly)), None);
    }

    #[test]
    fn onboarding_seen_parser_handles_valid_and_invalid_values() {
        assert!(parse_onboarding_seen("1"));
        assert!(parse_onboarding_seen(" 1\n"));
        assert!(parse_onboarding_seen("true"));
        assert!(parse_onboarding_seen("SEEN"));
        assert!(parse_onboarding_seen("Yes"));

        assert!(!parse_onboarding_seen(""));
        assert!(!parse_onboarding_seen("0"));
        assert!(!parse_onboarding_seen("false"));
        assert!(!parse_onboarding_seen("no"));
        assert!(!parse_onboarding_seen("corrupted"));
    }

    #[test]
    fn onboarding_state_persists_to_disk_and_round_trips() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "rongyok-ssb-test-{}-{unique}",
            std::process::id()
        ));
        let marker = dir.join("nested").join(ONBOARDING_STATE_FILE);

        assert!(!read_onboarding_seen(&marker));
        write_onboarding_seen(&marker).expect("write onboarding state");
        assert!(read_onboarding_seen(&marker));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn onboarding_only_auto_opens_on_first_interactive_launch() {
        assert!(should_show_onboarding(false, false));
        assert!(!should_show_onboarding(false, true));
        assert!(!should_show_onboarding(true, false));
        assert!(!should_show_onboarding(true, true));
    }

    #[test]
    fn onboarding_dismiss_url_is_recognized_and_not_treated_as_internal() {
        let dismiss_url = format!("{ONBOARDING_DISMISS_SCHEME}://{ONBOARDING_DISMISS_HOST}");
        let dismiss = Url::parse(&dismiss_url).unwrap();
        assert!(is_onboarding_dismiss_url(&dismiss));
        assert!(!is_internal(&dismiss));

        assert!(!is_onboarding_dismiss_url(
            &Url::parse("rongyok-desk://other").unwrap()
        ));
        assert!(!is_onboarding_dismiss_url(
            &Url::parse("https://rongyok.com/onboarding-dismissed").unwrap()
        ));
        assert!(!is_onboarding_dismiss_url(
            &Url::parse("https://rongyok-desk.example/").unwrap()
        ));
    }

    #[test]
    fn onboarding_auto_open_waits_for_finished_internal_http_load() {
        let home = Url::parse("https://rongyok.com/").unwrap();
        let blank = Url::parse("about:blank").unwrap();
        let foreign = Url::parse("https://example.com/").unwrap();

        assert!(should_auto_open_on_page_load(
            PageLoadEvent::Finished,
            &home,
            true
        ));
        assert!(!should_auto_open_on_page_load(
            PageLoadEvent::Started,
            &home,
            true
        ));
        assert!(!should_auto_open_on_page_load(
            PageLoadEvent::Finished,
            &blank,
            true
        ));
        assert!(!should_auto_open_on_page_load(
            PageLoadEvent::Finished,
            &foreign,
            true
        ));
        assert!(!should_auto_open_on_page_load(
            PageLoadEvent::Finished,
            &home,
            false
        ));
    }

    #[test]
    fn shortcut_script_contains_isolated_modal_all_shortcuts_and_no_emoji() {
        let dismiss_url = format!("{ONBOARDING_DISMISS_SCHEME}://{ONBOARDING_DISMISS_HOST}");
        assert!(SHORTCUT_SCRIPT.contains("attachShadow({ mode: 'closed' })"));
        assert!(SHORTCUT_SCRIPT.contains(r#":host([data-rongyok-open="true"])"#));
        assert!(SHORTCUT_SCRIPT.contains("z-index: 2147483647 !important"));
        assert!(SHORTCUT_SCRIPT.contains("svg { pointer-events: none; }"));
        assert!(SHORTCUT_SCRIPT.contains("keys-cell"));
        assert!(SHORTCUT_SCRIPT.contains("overflow-wrap: anywhere"));
        assert!(SHORTCUT_SCRIPT.contains(r#"grid-template-areas: "action keys" "detail detail""#));
        assert!(SHORTCUT_SCRIPT.contains("#120e18"));
        assert!(SHORTCUT_SCRIPT.contains("#e4b65e"));
        assert!(SHORTCUT_SCRIPT.contains(&dismiss_url));
        assert!(SHORTCUT_SCRIPT.contains("Alt"));
        assert!(SHORTCUT_SCRIPT.contains("Left"));
        assert!(SHORTCUT_SCRIPT.contains("Right"));
        assert!(SHORTCUT_SCRIPT.contains("F5"));
        assert!(SHORTCUT_SCRIPT.contains("Ctrl+R"));
        assert!(SHORTCUT_SCRIPT.contains("Cmd+R"));
        assert!(SHORTCUT_SCRIPT.contains("Go to rongyok.com"));
        assert!(SHORTCUT_SCRIPT.contains("F11"));
        assert!(SHORTCUT_SCRIPT.contains("Open this page in browser"));
        assert!(SHORTCUT_SCRIPT.contains("Menu only"));
        assert!(SHORTCUT_SCRIPT.contains("Quit"));
        assert!(SHORTCUT_SCRIPT.contains("Got it"));
        assert!(SHORTCUT_SCRIPT.contains("key === 'escape'"));
        assert!(SHORTCUT_SCRIPT.contains("key === 'f1'"));
        assert!(SHORTCUT_SCRIPT.contains("code === 'Slash'"));
        assert!(SHORTCUT_SCRIPT.is_ascii());
    }

    #[test]
    fn about_version_info_uses_compile_time_version_and_platform() {
        let info = version_info_string();
        let expected_version = format!("v{}", env!("CARGO_PKG_VERSION"));
        assert!(info.starts_with("Rongyok Desk "));
        assert!(info.contains(&expected_version));
        assert!(info.contains(std::env::consts::OS));
        assert!(info.contains(std::env::consts::ARCH));
        assert!(info.is_ascii());
    }

    #[test]
    fn js_single_quoted_escaper_handles_quotes_backslashes_and_newlines() {
        assert_eq!(
            escape_js_single_quoted("a'b\\c\nd\re"),
            r"a\'b\\c\nd\re"
        );
    }

    #[test]
    fn about_script_contains_isolated_modal_real_version_and_no_emoji() {
        let script = about_script();
        let expected_version = format!("v{}", env!("CARGO_PKG_VERSION"));

        assert!(script.is_ascii());
        assert!(script.contains("rongyok-desk-about-host"));
        assert!(script.contains("attachShadow({ mode: 'closed' })"));
        assert!(script.contains(r#":host([data-rongyok-open="true"])"#));
        assert!(script.contains("z-index: 2147483647 !important"));
        assert!(script.contains("svg { pointer-events: none; }"));
        assert!(script.contains("#120e18"));
        assert!(script.contains("#e4b65e"));
        assert!(script.contains("@media (max-width: 440px)"));
        assert!(script.contains("Rongyok Desk"));
        assert!(script.contains(&expected_version));
        assert!(script.contains(&version_info_string()));
        assert!(script.contains(START_URL));
        assert!(script.contains(APP_COPYRIGHT_JS));
        assert!(script.contains("Copy Version Info"));
        assert!(script.contains("Copied!"));
        assert!(script.contains("key === 'escape'"));
        assert!(script.contains("window.__rongyokShowAbout"));
        assert!(script.contains("window.__rongyokCloseAbout"));
        assert!(script.contains("window.__rongyokCloseShortcuts"));
        assert!(!script.contains("__APP_NAME__"));
        assert!(!script.contains("__VERSION_BADGE__"));
        assert!(!script.contains("__VERSION_INFO__"));
        assert!(!script.contains("__COPYRIGHT__"));
    }
}