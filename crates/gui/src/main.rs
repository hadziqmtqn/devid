//! Devid's desktop app (Phase 7, D-023, D-026): a menu-bar icon, plus a window that is
//! created when opened and destroyed when closed. Like the CLI, it is only a daemon client.

mod api;
mod tray;

use std::sync::Mutex;
use tauri::{AppHandle, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};

/// D-026: the window opens at this size, shrunk to fit the screen, and never gets smaller
/// than the minimum.
const SIZE: (f64, f64) = (1080.0, 680.0);
const MIN: (f64, f64) = (760.0, 520.0);

/// The app binary this process runs, as (device, inode). `devid setup` swaps the whole bundle on
/// an update, so the same path then names another file.
static RUNNING: Mutex<Option<(u64, u64)>> = Mutex::new(None);

fn main() {
    *RUNNING.lock().unwrap() = binary_id();
    // `--background` starts with the menu-bar icon only, for launch at login (G3); `--page
    // settings` opens the window on that page (after an update from the app).
    let background = std::env::args().any(|a| a == "--background");
    let page = std::env::args().skip_while(|a| a != "--page").nth(1).and_then(|p| start_page(&p));
    let app = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            api::daemon,
            api::open,
            api::copy_url,
            api::copy_log_path,
            api::server_config,
            api::choose_folder,
            api::doctor,
            api::devid_check,
            api::in_terminal,
            api::login_item,
            api::self_update,
            api::relaunch,
            api::quit,
        ])
        .setup(move |app| {
            // Menu bar only while no window is open; in the Dock and Cmd+Tab with one (D-030).
            #[cfg(target_os = "macos")]
            app.set_activation_policy(if background {
                tauri::ActivationPolicy::Accessory
            } else {
                tauri::ActivationPolicy::Regular
            });
            tray::start(app)?;
            if !background {
                open_window(app.handle(), page);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("cannot start the Devid app");

    app.run(|_app, event| match event {
        // The last window closed. Quit passes an exit code and is not caught here.
        // Restarting gives the memory the window used back to the system: about 20 MB, mostly
        // graphics, stays in the process after the window and its web view are gone (D-027).
        RunEvent::ExitRequested { code: None, api, .. } => {
            api.prevent_exit();
            restart(&["--background"]);
        }
        // Opened again from Finder or Spotlight while running.
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => open_window(_app, None),
        _ => {}
    });
}

/// Shows the window, creating it (and its web view) if it is closed, on `page` if given
/// ("settings" from the menu bar).
pub fn open_window(app: &AppHandle, page: Option<&'static str>) {
    // With a window, Devid is in the Dock and in Cmd+Tab with its icon (D-030). Closing the window
    // restarts the app as icon-only (D-027), which takes it out of both again.
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.set_focus();
        if let Some(page) = page {
            let _ = window.eval(format!("go('{page}')"));
        }
        return;
    }
    // Updated while only the menu-bar icon ran (the icon checks every 15 s): open the window in
    // the new version, not in this one.
    if updated() {
        restart(&[]);
    }
    let (width, height) = fit(app);
    let builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
        .title("Devid")
        .inner_size(width, height)
        .min_inner_size(MIN.0, MIN.1)
        .center();
    // Before app.js runs: this app's version (to compare with the daemon's, G4) and the first page.
    let builder = builder.initialization_script(format!(
        "window.DEVID_APP_VERSION = '{}'; window.DEVID_START_PAGE = '{}';",
        env!("CARGO_PKG_VERSION"),
        page.unwrap_or("")
    ));
    // System Settings look: the sidebar runs under the traffic lights.
    #[cfg(target_os = "macos")]
    let builder = builder.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true);
    match builder.build() {
        Ok(window) => {
            let _ = window.set_focus();
        }
        Err(e) => eprintln!("devid-gui: cannot open the window: {e}"),
    }
}

fn binary_id() -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(std::env::current_exe().ok()?).ok()?;
    Some((meta.dev(), meta.ino()))
}

/// Whether another app binary was installed over this one since it started. Not while the bundle
/// is being swapped (no file at the path), and never because the daemon's version differs: a
/// failed app install would otherwise restart the same binary forever.
pub fn updated() -> bool {
    replaced(*RUNNING.lock().unwrap(), binary_id())
}

fn replaced(running: Option<(u64, u64)>, now: Option<(u64, u64)>) -> bool {
    running.is_some() && now.is_some() && now != running
}

/// One of the window's pages, by name.
pub fn start_page(name: &str) -> Option<&'static str> {
    ["sites", "php", "server", "logs", "settings"].into_iter().find(|p| *p == name)
}

/// Replaces this process with a fresh copy of the app binary at the same path (same pid), with
/// `args`: `--background` for the menu-bar icon only, none for its window. Returns only if that
/// fails; the app then keeps running as it is and does not try again for this binary.
pub fn restart(args: &[&str]) {
    use std::os::unix::process::CommandExt;
    let err = match std::env::current_exe() {
        Ok(exe) => std::process::Command::new(exe).args(args).exec(),
        Err(e) => e,
    };
    eprintln!("devid-gui: cannot restart: {err}");
    *RUNNING.lock().unwrap() = binary_id();
}

/// The default size, shrunk to the primary screen's visible area (without the menu bar
/// and the Dock) minus a margin.
fn fit(app: &AppHandle) -> (f64, f64) {
    let Ok(Some(monitor)) = app.primary_monitor() else {
        return SIZE;
    };
    let area = monitor.work_area().size.to_logical::<f64>(monitor.scale_factor());
    clamp(SIZE, (area.width - 40.0, area.height - 40.0))
}

fn clamp(want: (f64, f64), room: (f64, f64)) -> (f64, f64) {
    (want.0.min(room.0).max(MIN.0), want.1.min(room.1).max(MIN.1))
}

#[cfg(test)]
mod tests {
    #[test]
    fn restarts_only_for_another_binary_at_the_path() {
        let replaced = super::replaced;
        assert!(replaced(Some((1, 10)), Some((1, 11))), "the bundle was swapped");
        assert!(!replaced(Some((1, 10)), Some((1, 10))), "same binary");
        assert!(!replaced(Some((1, 10)), None), "mid-swap: no file at the path yet");
        assert!(!replaced(None, Some((1, 11))), "unknown at start");
    }

    use super::*;

    #[test]
    fn window_size_fits_the_screen_but_not_below_the_minimum() {
        assert_eq!(clamp(SIZE, (1400.0, 830.0)), SIZE, "a 1440×900 screen fits the default");
        assert_eq!(clamp(SIZE, (1240.0, 735.0)), (1080.0, 680.0), "1280×800 still fits");
        assert_eq!(clamp(SIZE, (984.0, 700.0)), (984.0, 680.0), "1024×768 shrinks the width");
        assert_eq!(clamp(SIZE, (600.0, 400.0)), MIN, "never below the minimum");
    }
}
