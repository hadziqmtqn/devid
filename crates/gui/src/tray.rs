//! The menu-bar icon and its menu (Phase 7 G3): the state at a glance, Start all, Stop all and
//! Restart, available updates (D-028), running commands with Stop (Phase 8 A5, D-036), sleeping
//! on-demand sites, which a click opens and so wakes (Phase 10, D-054), Mail (Phase 9, D-049),
//! Open Devid, Settings and Quit.

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
/// While Mail runs with notifications on (D-050): a new message shows up within 5 s.
const EVERY_MAIL: Duration = Duration::from_secs(5);

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
    /// On-demand sites whose command waits for the next visit (D-054).
    pub sleeping: Vec<String>,
    /// Shared sites (D-037).
    pub shares: Vec<String>,
    /// Mail (D-049): None while it is off.
    pub mail: Option<MailView>,
    /// The databases that are on (D-061), as (label, 127.0.0.1 port while running).
    pub databases: Vec<(String, Option<u64>)>,
}

/// Mail as the icon sees it (D-049, D-050).
#[derive(Debug, Clone, PartialEq)]
pub struct MailView {
    pub running: bool,
    pub unread: u64,
    pub notify: bool,
    /// The newest message, from `status.mail.latest`.
    pub latest: Option<Latest>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Latest {
    /// When Mailpit received it (RFC 3339, so later sorts later).
    pub created: String,
    pub from: String,
    pub subject: String,
}

/// What to post since the icon last looked at Mail (D-050), as (title, subtitle, text): a first
/// notification when they were just turned on, so macOS asks for permission right then, or the
/// newest message when it is newer than the last one seen, with the sender as the title, as Mail.app
/// does, and the subject; nothing more, which was too much (the owner, 2026-11-16). Nothing at the
/// first look, so old mail stays quiet when the app starts.
pub fn mail_news(before: Option<&MailView>, now: Option<&MailView>) -> Option<(String, String, String)> {
    let (before, now) = (before?, now?);
    if !now.notify || !now.running {
        return None;
    }
    if !before.notify {
        return Some(("Devid Mail".into(), "Notifications are on".into(), String::new()));
    }
    let latest = now.latest.as_ref()?;
    // shortcut: compares Mailpit's local-time strings; in the hour after a fall-back to winter time a new
    // message can look older and stays quiet. Parse the times if that ever matters.
    if before.latest.as_ref().is_some_and(|b| b.created >= latest.created) {
        return None;
    }
    let subject = if latest.subject.is_empty() { "(no subject)".into() } else { latest.subject.clone() };
    Some((latest.from.clone(), subject, String::new()))
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
            sleeping: Vec::new(),
            shares: Vec::new(),
            mail: None,
            databases: Vec::new(),
        };
    };
    let failed = s["php"].as_array().into_iter().flatten().filter(|p| p["status"] == "failed").count()
        + usize::from(s["server_status"] == "failed");
    let running = s["server_status"] == "running";
    // D-047: another app switched pf off on loopback; the server runs, but no site answers.
    let ports_off = s["standard_ports_ok"] == false;
    let server = s["server"].as_str().unwrap_or("?").to_string();
    let header = if failed > 0 {
        format!("{failed} service{} failed", if failed == 1 { "" } else { "s" })
    } else if ports_off {
        "Sites do not answer on ports 80 and 443".into()
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
    let sleeping =
        s["sleeping"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
    let shares =
        s["shares"].as_array().into_iter().flatten().filter_map(|x| x["site"].as_str().map(String::from)).collect();
    let m = &s["mail"];
    let text = |v: &Value| v.as_str().unwrap_or_default().to_string();
    let mail = (m["enabled"] == true).then(|| MailView {
        running: m["state"] == "running",
        unread: m["unread"].as_u64().unwrap_or(0),
        notify: m["notify"] == true,
        latest: m["latest"]["created"].as_str().map(|created| Latest {
            created: created.to_string(),
            from: text(&m["latest"]["from"]),
            subject: text(&m["latest"]["subject"]),
        }),
    });
    let databases = s["databases"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|d| d["enabled"] == true)
        .map(|d| {
            let label = d["label"].as_str().unwrap_or("?").to_string();
            (label, d["port"].as_u64().filter(|_| d["state"] == "running"))
        })
        .collect();
    View {
        icon: if failed > 0 || ports_off {
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
        sleeping,
        shares,
        mail,
        databases,
    }
}

/// "Commands · 2 running · 1 sleeping"; None when nothing runs or sleeps.
fn commands_header(v: &View) -> Option<String> {
    let (running, sleeping) = (v.commands.len(), v.sleeping.len());
    match (running, sleeping) {
        (0, 0) => None,
        (r, 0) => Some(format!("Commands · {r} running")),
        (0, z) => Some(format!("Commands · {z} sleeping")),
        (r, z) => Some(format!("Commands · {r} running · {z} sleeping")),
    }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// The last shown view (to skip redraws), the last action's error (shown once in the menu), and
/// Mail as last seen, for new-mail notifications.
#[derive(Default)]
struct TrayState {
    shown: Mutex<String>,
    error: Mutex<Option<String>>,
    mail: Mutex<Option<MailView>>,
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
            let mail_watch = refresh(&handle);
            std::thread::sleep(if mail_watch { EVERY_MAIL } else { EVERY });
        }
    });
    Ok(())
}

/// Asks the daemon for its state, posts a notification for new mail, and redraws the icon and the
/// menu if anything changed. True while Mail runs with notifications on (check more often).
pub fn refresh(app: &AppHandle) -> bool {
    // A self-update installed a new app while only this icon ran: become the new version now,
    // so the menu and the next window are current (D-027 restarts the same way).
    if crate::updated() && app.get_webview_window("main").is_none() {
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            if handle.get_webview_window("main").is_none() {
                crate::restart(&handle, &["--background"]);
            }
        });
        return false;
    }
    let status = request(&Paths::from_env(), &Request::Status);
    let v = view(status.as_ref().map_err(String::as_str));
    let state = app.state::<TrayState>();
    {
        let mut seen = state.mail.lock().unwrap();
        if let Some((title, subtitle, text)) = mail_news(seen.as_ref(), v.mail.as_ref()) {
            crate::notify::post(app, title, subtitle, text);
        }
        *seen = v.mail.clone();
    }
    let mail_watch = v.mail.as_ref().is_some_and(|m| m.running && m.notify);
    let error = state.error.lock().unwrap().clone();
    let key = format!("{v:?}{error:?}");
    let mut shown = state.shown.lock().unwrap();
    if *shown == key {
        return mail_watch;
    }
    let Some(tray) = app.tray_by_id(ID) else { return mail_watch };
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
    mail_watch
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
    if let Some(header) = commands_header(v) {
        m.append(&sep()?)?;
        m.append(&item("commands", &header, false, None)?)?;
        for (site, name) in &v.commands {
            m.append(&item(&format!("cmd-stop:{site}/{name}"), &format!("Stop {site} · {name}"), true, None)?)?;
        }
        // D-054: opening a sleeping site is what wakes its command.
        for site in &v.sleeping {
            m.append(&item(&format!("site-open:{site}"), &format!("Open {site} (sleeping)"), true, None)?)?;
        }
        if !v.commands.is_empty() {
            m.append(&item("cmd-stop-all", "Stop all commands", true, None)?)?;
        }
    }
    if !v.shares.is_empty() {
        m.append(&sep()?)?;
        for site in &v.shares {
            m.append(&item(&format!("share-stop:{site}"), &format!("Stop sharing {site}"), true, None)?)?;
        }
    }
    if v.running.is_some() {
        m.append(&sep()?)?;
        match v.mail.as_ref().map(|m| (m.running, m.unread)) {
            Some((true, unread)) => {
                let header = if unread > 0 { format!("Mail · {unread} new") } else { "Mail".into() };
                m.append(&item("mail", &header, false, None)?)?;
                m.append(&item("mail-open", "Open inbox", true, None)?)?;
                m.append(&item("mail-test", "Send test email", true, None)?)?;
            }
            Some((false, _)) => {
                m.append(&item("mail", "Mail · Stopped", false, None)?)?;
                m.append(&item("mail-page", "Mail…", true, None)?)?;
            }
            None => m.append(&item("mail-page", "Turn on Mail…", true, None)?)?,
        }
        if !v.databases.is_empty() {
            m.append(&sep()?)?;
            m.append(&item("databases", "Databases", false, None)?)?;
            for (label, port) in &v.databases {
                let text = match port {
                    Some(port) => format!("{label} · 127.0.0.1:{port}"),
                    None => format!("{label} · Stopped"),
                };
                m.append(&item(&format!("db-page:{label}"), &text, true, None)?)?;
            }
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
        // The inbox is the window's Mail page (D-051).
        "mail-page" | "mail-open" => crate::open_window(app, Some("mail")),
        id if id.starts_with("db-page:") => crate::open_window(app, Some("databases")),
        "mail-test" => {
            let app = app.clone();
            std::thread::spawn(move || {
                *app.state::<TrayState>().error.lock().unwrap() = request(&Paths::from_env(), &Request::MailTest).err();
                refresh(&app);
            });
        }
        "quit" => app.exit(0),
        id if id.starts_with("site-open:") => {
            let (app, site) = (app.clone(), id.trim_start_matches("site-open:").to_string());
            std::thread::spawn(move || {
                // The full path, as in api::open: a terminal's own `open` can come first on PATH.
                let opened = crate::api::site_url(&Paths::from_env(), &site).and_then(|url| {
                    std::process::Command::new("/usr/bin/open").arg(url).status().map(|_| ()).map_err(|e| e.to_string())
                });
                *app.state::<TrayState>().error.lock().unwrap() = opened.err();
                refresh(&app);
            });
        }
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

        let ports_off = json!({"server": "nginx", "server_status": "running", "default_php": "8.5",
            "php": [{"status": "running"}], "updates": null, "standard_ports_ok": false});
        let v = view(Ok(&ports_off));
        assert_eq!((v.icon, v.header.as_str()), ("bad", "Sites do not answer on ports 80 and 443"), "D-047");

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
        assert_eq!(view(Ok(&shared)).mail, None, "Mail is off");
        let mail = json!({"server": "nginx", "server_status": "running", "php": [],
            "mail": {"enabled": true, "state": "running", "unread": 3}});
        let m = view(Ok(&mail)).mail.unwrap();
        assert_eq!((m.running, m.unread, m.notify, m.latest), (true, 3, false, None));
        let paused = json!({"server": "nginx", "server_status": "running", "php": [],
            "mail": {"enabled": true, "state": "paused", "unread": null}});
        assert_eq!(view(Ok(&paused)).mail.map(|m| (m.running, m.unread)), Some((false, 0)));
        // D-061: the databases that are on, with their port while they run.
        let dbs = json!({"server": "nginx", "server_status": "running", "php": [], "databases": [
            {"label": "MySQL", "enabled": true, "state": "running", "port": 3307},
            {"label": "PostgreSQL", "enabled": true, "state": "paused", "port": 5433},
            {"label": "MongoDB", "enabled": false, "state": "off", "port": null}]});
        assert_eq!(
            view(Ok(&dbs)).databases,
            [("MySQL".to_string(), Some(3307)), ("PostgreSQL".to_string(), None)],
            "an engine that is off is not listed"
        );
        assert!(view(Ok(&running)).databases.is_empty(), "a daemon without databases");
        assert_eq!(
            view(Ok(&commands)).commands,
            [("erd".to_string(), "dev:pg:local".to_string()), ("erd".to_string(), "tauri".to_string())]
        );

        // D-054: sleeping on-demand sites, counted apart from running commands.
        assert_eq!(commands_header(&view(Ok(&running))), None);
        assert_eq!(commands_header(&view(Ok(&commands))).as_deref(), Some("Commands · 2 running"));
        let mut sleeping = commands.clone();
        sleeping["sleeping"] = json!(["devid-site"]);
        let v = view(Ok(&sleeping));
        assert_eq!(v.sleeping, ["devid-site"]);
        assert_eq!(commands_header(&v).as_deref(), Some("Commands · 2 running · 1 sleeping"));
        sleeping["running_commands"] = json!([]);
        assert_eq!(commands_header(&view(Ok(&sleeping))).as_deref(), Some("Commands · 1 sleeping"));
    }

    #[test]
    fn new_mail_notifications() {
        let latest = |created: &str| {
            Some(Latest {
                created: created.into(),
                from: "ERD Builder Pro".into(),
                subject: "Reset Password Notification".into(),
            })
        };
        let mail = |notify, running, l: Option<Latest>| MailView { running, unread: 1, notify, latest: l };
        let a = mail(true, true, latest("2026-11-16T14:00:00Z"));
        assert_eq!(mail_news(None, Some(&a)), None, "nothing at the first look, when the app starts");
        assert_eq!(mail_news(Some(&a), Some(&a)), None, "the same newest message");
        let b = mail(true, true, latest("2026-11-16T14:00:05Z"));
        let posted = mail_news(Some(&a), Some(&b)).unwrap();
        assert_eq!(
            (posted.0.as_str(), posted.1.as_str(), posted.2.as_str()),
            ("ERD Builder Pro", "Reset Password Notification", ""),
            "the sender as the title, as Mail.app shows it, and the subject"
        );
        assert_eq!(mail_news(Some(&b), Some(&a)), None, "the newest was deleted: an older one is not new");
        assert_eq!(
            mail_news(Some(&mail(true, true, None)), Some(&a)).map(|p| p.0),
            Some("ERD Builder Pro".into()),
            "the first message in an empty inbox"
        );
        let off = mail(false, true, latest("2026-11-16T14:00:00Z"));
        assert_eq!(mail_news(Some(&off), Some(&b)).map(|p| p.0), Some("Devid Mail".into()), "turned on: ask macOS now");
        assert_eq!(
            mail_news(Some(&a), Some(&mail(false, true, latest("2026-11-16T14:00:09Z")))),
            None,
            "notifications off"
        );
        assert_eq!(mail_news(Some(&a), Some(&mail(true, false, latest("2026-11-16T14:00:09Z")))), None, "Mail stopped");
        assert_eq!(mail_news(Some(&a), None), None, "Mail turned off");
        let mut untitled = b.clone();
        untitled.latest.as_mut().unwrap().subject.clear();
        assert_eq!(mail_news(Some(&a), Some(&untitled)).map(|p| p.1), Some("(no subject)".into()));
    }
}
