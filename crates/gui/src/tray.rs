//! The menu-bar icon and its menu (Phase 7 G3): the state at a glance, Start all, Stop all and
//! Restart, available updates (D-028), running commands with Stop (Phase 8 A5, D-036), Open Devid,
//! Settings and Quit.

use devid_daemon::{
    paths::Paths,
    protocol::{Request, request},
};
use serde_json::Value;
use std::{sync::Mutex, time::Duration};
use tauri::{
    AppHandle, Manager, Wry,
    image::Image,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};

const ID: &str = "devid";
/// How often the icon asks the daemon for its state: one small socket request, so idle stays
/// at about 0% CPU (the G1 limit).
const EVERY: Duration = Duration::from_secs(15);

/// What the icon and the menu show.
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    /// "running", "off" (stopped, or no daemon) or "bad" (something failed).
    pub icon: &'static str,
    /// The first line of the menu, as on the window's status card.
    pub header: String,
    /// None: no daemon to send Start or Stop to.
    pub running: Option<bool>,
    pub server: String,
    /// A newer devid plus the PHP, web server and extension updates the last check found.
    pub updates: usize,
    /// Running commands (D-036) as (site, name); their number shows next to the icon.
    pub commands: Vec<(String, String)>,
    /// Shared sites (D-037).
    pub shares: Vec<String>,
}

pub fn view(status: Result<&Value, &str>) -> View {
    let Ok(s) = status else {
        return View {
            icon: "off",
            header: "Devid is not running".into(),
            running: None,
            server: String::new(),
            updates: 0,
            commands: Vec::new(),
            shares: Vec::new(),
        };
    };
    let failed = s["php"].as_array().into_iter().flatten().filter(|p| p["status"] == "failed").count()
        + usize::from(s["server_status"] == "failed");
    let running = s["server_status"] == "running";
    let server = s["server"].as_str().unwrap_or("?").to_string();
    let header = if failed > 0 {
        format!("{failed} service{} failed", if failed == 1 { "" } else { "s" })
    } else if running {
        format!("{server} · Running · PHP {}", s["default_php"].as_str().unwrap_or("?"))
    } else {
        format!("{server} · {}", capital(s["server_status"].as_str().unwrap_or("stopped")))
    };
    let u = &s["updates"];
    let updates = u["changes"].as_array().map_or(0, Vec::len) + usize::from(u["devid"].is_string());
    let commands = s["running_commands"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| (c["site"].as_str().unwrap_or("?").to_string(), c["name"].as_str().unwrap_or("?").to_string()))
        .collect();
    let shares =
        s["shares"].as_array().into_iter().flatten().filter_map(|x| x["site"].as_str().map(String::from)).collect();
    View {
        icon: if failed > 0 {
            "bad"
        } else if running {
            "running"
        } else {
            "off"
        },
        header,
        running: Some(running),
        server,
        updates,
        commands,
        shares,
    }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// The last shown view (to skip redraws) and the last action's error (shown once in the menu).
#[derive(Default)]
struct TrayState {
    shown: Mutex<String>,
    error: Mutex<Option<String>>,
}

pub fn start(app: &tauri::App) -> tauri::Result<()> {
    app.manage(TrayState::default());
    let first = view(Err(""));
    TrayIconBuilder::with_id(ID)
        // Template images: macOS tints them for light, dark and highlighted menu bars.
        .icon(icon(&first))
        .icon_as_template(true)
        .tooltip("Devid")
        .menu(&menu(app.handle(), &first, None)?)
        .on_menu_event(on_menu)
        .build(app)?;
    let handle = app.handle().clone();
    std::thread::spawn(move || {
        loop {
            refresh(&handle);
            std::thread::sleep(EVERY);
        }
    });
    Ok(())
}

/// Asks the daemon for its state and redraws the icon and the menu if anything changed.
pub fn refresh(app: &AppHandle) {
    // A self-update installed a new app while only this icon ran: become the new version now,
    // so the menu and the next window are current (D-027 restarts the same way).
    if crate::updated() && app.get_webview_window("main").is_none() {
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            if handle.get_webview_window("main").is_none() {
                crate::restart(true);
            }
        });
        return;
    }
    let status = request(&Paths::from_env(), &Request::Status);
    let v = view(status.as_ref().map_err(String::as_str));
    let state = app.state::<TrayState>();
    let error = state.error.lock().unwrap().clone();
    let key = format!("{v:?}{error:?}");
    let mut shown = state.shown.lock().unwrap();
    if *shown == key {
        return;
    }
    let Some(tray) = app.tray_by_id(ID) else { return };
    match menu(app, &v, error.as_deref()) {
        Ok(m) => {
            let _ = tray.set_menu(Some(m));
        }
        Err(e) => eprintln!("devid-gui: menu: {e}"),
    }
    let _ = tray.set_icon(Some(icon(&v)));
    let count = v.commands.len().to_string();
    let _ = tray.set_title((!v.commands.is_empty()).then_some(count.as_str()));
    let _ = tray.set_icon_as_template(true);
    let _ = tray.set_tooltip(Some(&v.header));
    *shown = key;
}

fn icon(v: &View) -> Image<'static> {
    match (v.icon, v.updates > 0) {
        ("running", false) => tauri::include_image!("icons/tray.png"),
        ("running", true) => tauri::include_image!("icons/tray-dot.png"),
        ("bad", false) => tauri::include_image!("icons/tray-bad.png"),
        ("bad", true) => tauri::include_image!("icons/tray-bad-dot.png"),
        (_, false) => tauri::include_image!("icons/tray-off.png"),
        (_, true) => tauri::include_image!("icons/tray-off-dot.png"),
    }
}

fn menu(app: &AppHandle, v: &View, error: Option<&str>) -> tauri::Result<Menu<Wry>> {
    let m = Menu::new(app)?;
    let item = |id: &str, text: &str, enabled: bool, key: Option<&str>| MenuItem::with_id(app, id, text, enabled, key);
    let sep = || PredefinedMenuItem::separator(app);
    m.append(&item("status", &v.header, false, None)?)?;
    if let Some(e) = error {
        m.append(&item("error", &format!("⚠ {e}"), false, None)?)?;
    }
    if v.updates > 0 {
        m.append(&item("updates", &format!("Updates available ({})…", v.updates), true, None)?)?;
    }
    if !v.commands.is_empty() {
        m.append(&sep()?)?;
        let n = v.commands.len();
        m.append(&item("commands", &format!("Commands · {n} running"), false, None)?)?;
        for (site, name) in &v.commands {
            m.append(&item(&format!("cmd-stop:{site}/{name}"), &format!("Stop {site} · {name}"), true, None)?)?;
        }
        m.append(&item("cmd-stop-all", "Stop all commands", true, None)?)?;
    }
    if !v.shares.is_empty() {
        m.append(&sep()?)?;
        for site in &v.shares {
            m.append(&item(&format!("share-stop:{site}"), &format!("Stop sharing {site}"), true, None)?)?;
        }
    }
    match v.running {
        Some(true) => {
            m.append(&sep()?)?;
            m.append(&item("stop", "Stop all", true, None)?)?;
            m.append(&item("restart", &format!("Restart {}", v.server), true, None)?)?;
        }
        Some(false) => {
            m.append(&sep()?)?;
            m.append(&item("start", "Start all", true, None)?)?;
        }
        None => {}
    }
    m.append(&sep()?)?;
    m.append(&item("open", "Open Devid", true, None)?)?;
    m.append(&item("settings", "Settings…", true, None)?)?;
    m.append(&sep()?)?;
    m.append(&item("quit", "Quit Devid", true, Some("CmdOrCtrl+Q"))?)?;
    Ok(m)
}

fn on_menu(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "open" => crate::open_window(app, None),
        "settings" | "updates" => crate::open_window(app, Some("settings")),
        "quit" => app.exit(0),
        id if id.starts_with("share-stop:") => {
            let (app, site) = (app.clone(), id.trim_start_matches("share-stop:").to_string());
            std::thread::spawn(move || {
                let req = Request::ShareStop { site: Some(site), cwd: "/".into() };
                *app.state::<TrayState>().error.lock().unwrap() = request(&Paths::from_env(), &req).err();
                refresh(&app);
            });
        }
        id if id == "cmd-stop-all" || id.starts_with("cmd-stop:") => {
            // A stop waits until nothing of the command is left (up to 10 s).
            let (app, id) = (app.clone(), id.to_string());
            std::thread::spawn(move || {
                let paths = Paths::from_env();
                let stop = |site: &str, name: Option<&str>| {
                    let req =
                        Request::CommandStop { site: Some(site.into()), cwd: "/".into(), name: name.map(String::from) };
                    request(&paths, &req).err()
                };
                let error = match id.strip_prefix("cmd-stop:").and_then(|k| k.split_once('/')) {
                    Some((site, name)) => stop(site, Some(name)),
                    None => {
                        let status = request(&paths, &Request::Status).unwrap_or_default();
                        let sites: std::collections::BTreeSet<String> =
                            view(Ok(&status)).commands.into_iter().map(|(s, _)| s).collect();
                        sites.iter().find_map(|site| stop(site, None))
                    }
                };
                *app.state::<TrayState>().error.lock().unwrap() = error;
                refresh(&app);
            });
        }
        id @ ("start" | "stop" | "restart") => {
            // Off the main thread: a request can take seconds (a restart waits for the server).
            let (app, id) = (app.clone(), id.to_string());
            std::thread::spawn(move || {
                let paths = Paths::from_env();
                let req = match id.as_str() {
                    "start" => Request::StartAll,
                    "stop" => Request::StopAll,
                    _ => {
                        let status = request(&paths, &Request::Status).unwrap_or_default();
                        Request::Restart { service: status["server"].as_str().unwrap_or("nginx").to_string() }
                    }
                };
                *app.state::<TrayState>().error.lock().unwrap() = request(&paths, &req).err();
                refresh(&app);
            });
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_view_follows_the_daemon() {
        let running = json!({"server": "nginx", "server_status": "running", "default_php": "8.5",
            "php": [{"status": "running"}], "updates": null});
        let v = view(Ok(&running));
        assert_eq!(
            (v.icon, v.header.as_str(), v.running, v.updates),
            ("running", "nginx · Running · PHP 8.5", Some(true), 0)
        );

        let stopped = json!({"server": "nginx", "server_status": "stopped", "php": [],
            "updates": {"devid": "0.6.0", "changes": [{"what": "php 8.5"}, {"what": "nginx"}]}});
        let v = view(Ok(&stopped));
        assert_eq!((v.icon, v.header.as_str(), v.running, v.updates), ("off", "nginx · Stopped", Some(false), 3));

        let failed = json!({"server": "nginx", "server_status": "running", "php": [{"status": "failed"}]});
        assert_eq!(view(Ok(&failed)).header, "1 service failed");
        assert_eq!(view(Ok(&failed)).icon, "bad");

        let down = view(Err("daemon is not running"));
        assert_eq!((down.icon, down.running), ("off", None));

        let commands = json!({"server": "nginx", "server_status": "running", "php": [],
            "running_commands": [{"site": "erd", "name": "dev:pg:local"}, {"site": "erd", "name": "tauri"}]});
        let shared = json!({"server": "nginx", "server_status": "running", "php": [],
            "shares": [{"site": "erd", "url": "https://a-b.trycloudflare.com"}]});
        assert_eq!(view(Ok(&shared)).shares, ["erd"]);
        assert_eq!(
            view(Ok(&commands)).commands,
            [("erd".to_string(), "dev:pg:local".to_string()), ("erd".to_string(), "tauri".to_string())]
        );
    }
}
