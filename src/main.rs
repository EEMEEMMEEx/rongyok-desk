#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

// Rongyok Desk — a dedicated single-site browser.
//
// One native window that shows https://rongyok.com/ and stays there: links that
// leave the site open in the person's normal browser instead, and the usual
// browser keys (Back, Forward, Reload) keep working.

use std::path::PathBuf;
use std::time::Duration;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::utils::config::BackgroundThrottlingPolicy;
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// The site this window is locked to.
const START_URL: &str = "https://rongyok.com/";
/// The session/cookies live beside this, so signing in survives a restart.
const WINDOW_LABEL: &str = "main";

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

/// The keys a browser window keeps working, whatever the page does with them.
/// This runs inside the page, before the site's own scripts, and only steps in for
/// the combinations a desktop app should own.
const SHORTCUT_SCRIPT: &str = r#"
(function () {
  if (window.__rongyokKeys) return;
  window.__rongyokKeys = true;
  window.addEventListener('keydown', function (event) {
    var key = (event.key || '').toLowerCase();
    var alt = event.altKey;
    var ctrl = event.ctrlKey || event.metaKey;
    if (alt && !ctrl && (key === 'arrowleft' || key === 'left')) {
      event.preventDefault();
      history.back();
    } else if (alt && !ctrl && (key === 'arrowright' || key === 'right')) {
      event.preventDefault();
      history.forward();
    } else if (!alt && !ctrl && (key === 'f5' || (ctrl && key === 'r'))) {
      event.preventDefault();
      location.reload();
    }
  }, true);
})();
"#;
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

    Menu::with_items(app, &[&navigate])
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
                "quit" => app.exit(0),
                _ => {}
            }
        })
        .setup(move |app| {
            let handle = app.handle().clone();

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
            .on_navigation(|url| {
                if is_internal(url) {
                    return true;
                }
                // A link leaving the site: hand it to the normal browser, keep this
                // window where it is. mailto:, tel: and the like go there too.
                open_in_system_browser(url.as_str());
                false
            })
            .on_new_window({
                let handle = handle.clone();
                move |url, _features| {
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
}