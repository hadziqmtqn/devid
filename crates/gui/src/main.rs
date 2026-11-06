//! Devid's desktop app (Phase 7, D-023, D-026): a menu-bar icon, plus a window that is
//! created when opened and destroyed when closed. Like the CLI, it is only a daemon client.

mod api;
mod tray;

use tauri::{AppHandle, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};

/// D-026: the window opens at this size, shrunk to fit the screen, and never gets smaller
/// than the minimum.
const SIZE: (f64, f64) = (1080.0, 680.0);
const MIN: (f64, f64) = (760.0, 520.0);

fn main() {
    // `--background` starts with the menu-bar icon only, for launch at login (G3).
    let background = std::env::args().any(|a| a == "--background");
    let app = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            api::daemon,
            api::open,
            api::copy_url,
            api::server_config,
            api::choose_folder,
            api::doctor,
            api::devid_check,
            api::in_terminal,
            api::login_item,
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
                open_window(app.handle(), None);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("cannot start the Devid app");

    app.run(|_app, event| match event {
        // The last window closed. Quit passes an exit code and is not caught here.
        RunEvent::ExitRequested { code: None, api, .. } => {
            api.prevent_exit();
            restart_in_background();
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
    let (width, height) = fit(app);
    let builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
        .title("Devid")
        .inner_size(width, height)
        .min_inner_size(MIN.0, MIN.1)
        .center();
    // A new window reads its first page before app.js runs.
    let builder = match page {
        Some(page) => builder.initialization_script(format!("window.DEVID_START_PAGE = '{page}';")),
        None => builder,
    };
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

/// Replaces this process with a fresh `--background` copy of itself (same pid), so the
/// memory the window used goes back to the system: about 20 MB, mostly graphics, stays
/// in the process after the window and its web view are gone (D-027). Returns only if
/// the restart fails; the app then keeps running with that memory.
fn restart_in_background() {
    use std::os::unix::process::CommandExt;
    let err = match std::env::current_exe() {
        Ok(exe) => std::process::Command::new(exe).arg("--background").exec(),
        Err(e) => e,
    };
    eprintln!("devid-gui: cannot restart after the window closed: {err}");
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
    use super::*;

    #[test]
    fn window_size_fits_the_screen_but_not_below_the_minimum() {
        assert_eq!(clamp(SIZE, (1400.0, 830.0)), SIZE, "a 1440×900 screen fits the default");
        assert_eq!(clamp(SIZE, (1240.0, 735.0)), (1080.0, 680.0), "1280×800 still fits");
        assert_eq!(clamp(SIZE, (984.0, 700.0)), (984.0, 680.0), "1024×768 shrinks the width");
        assert_eq!(clamp(SIZE, (600.0, 400.0)), MIN, "never below the minimum");
    }
}
