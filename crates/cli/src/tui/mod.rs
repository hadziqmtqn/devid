//! `devid` with no arguments: the TUI (Phase 4, D-012). It only renders daemon state and sends
//! daemon requests; it runs only while open and polls `status` every 2 s.

mod app;
mod ui;

use crate::request;
use app::{Action, App, Effect};
use devid_daemon::{paths::Paths, protocol::Request};
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use std::time::{Duration, Instant};

const REFRESH: Duration = Duration::from_secs(2);

fn refresh(app: &mut App, paths: &Paths) {
    match request(paths, &Request::Status) {
        Ok(status) => {
            let sites = request(paths, &Request::Sites).unwrap_or_default();
            app.sites = sites["sites"].as_array().cloned().unwrap_or_default();
            app.parked = sites["parked"]
                .as_array()
                .map(|l| l.iter().filter_map(|p| p.as_str().map(String::from)).collect())
                .unwrap_or_default();
            app.ignored = sites["ignored"]
                .as_array()
                .map(|l| l.iter().filter_map(|i| i["name"].as_str().map(String::from)).collect())
                .unwrap_or_default();
            app.servers = request(paths, &Request::ServerList)
                .ok()
                .and_then(|v| v["servers"].as_array().cloned())
                .unwrap_or_default();
            app.logs =
                request(paths, &Request::Logs).ok().and_then(|v| v["logs"].as_array().cloned()).unwrap_or_default();
            app.log_text = match app.selected_log() {
                Some(name) => request(paths, &Request::LogRead { name, offset: None })
                    .map(|v| v["text"].as_str().unwrap_or_default().to_string())
                    .unwrap_or_else(|e| format!("({e})")),
                None => String::new(),
            };
            app.status = Ok(status);
        }
        Err(e) => {
            app.status = Err(e);
            app.sites.clear();
            app.parked.clear();
        }
    }
}

/// Fetch a PHP version's catalog (from the release manifest) and extension list, and show them.
/// Only when the view opens, never on the 2 s refresh: the catalog needs the network.
fn load_extensions(app: &mut App, paths: &Paths, version: &str) {
    let catalog = request(paths, &Request::PhpExtCatalog { version: version.to_string() });
    let list = request(paths, &Request::PhpExtList { version: version.to_string() });
    match (catalog, list) {
        (Ok(c), Ok(l)) => app.open_extensions(version, &c, &l),
        (Err(e), _) | (_, Err(e)) => app.message = Some((format!("extensions: {e}"), true)),
    }
}

/// Carry out an effect; the result becomes the message line.
fn perform(effect: Effect, paths: &Paths) -> (String, bool) {
    use std::process::{Command, Stdio};
    let run = |cmd: &mut Command, done: String| match cmd.stdout(Stdio::null()).stderr(Stdio::null()).status() {
        Ok(s) if s.success() => (done, false),
        Ok(s) => (format!("failed ({s})"), true),
        Err(e) => (e.to_string(), true),
    };
    match effect {
        Effect::Daemon { request: req, done } => match request(paths, &req) {
            Ok(_) => (done, false),
            Err(e) => (e, true),
        },
        Effect::OpenUrl(url) => run(Command::new("open").arg(&url), format!("opened {url}")),
        Effect::OpenTerminal(path) => {
            run(Command::new("open").args(["-a", "Terminal"]).arg(&path), format!("opened {path} in Terminal"))
        }
        Effect::Update { check } => match request(paths, &Request::Update { check }) {
            Ok(v) => {
                let changes: Vec<String> = v["changes"]
                    .as_array()
                    .map(|l| {
                        l.iter()
                            .map(|c| {
                                format!(
                                    "{} {}→{}",
                                    c["what"].as_str().unwrap_or("?"),
                                    c["from"].as_str().unwrap_or("?"),
                                    c["to"].as_str().unwrap_or("?")
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let unknown: Vec<&str> = v["unknown"]
                    .as_array()
                    .map(|l| l.iter().filter_map(serde_json::Value::as_str).collect())
                    .unwrap_or_default();
                let (text, failed) = match (changes.is_empty(), check) {
                    (true, _) if unknown.is_empty() => {
                        (format!("everything is up to date (release {})", v["release"].as_str().unwrap_or("?")), false)
                    }
                    (true, _) => ("no updates found".to_string(), true),
                    (false, true) => (format!("available: {} — press U to update", changes.join(", ")), false),
                    (false, false) => (format!("updated: {}", changes.join(", ")), false),
                };
                if unknown.is_empty() {
                    (text, failed)
                } else {
                    (format!("{text} · version unknown, skipped: {} (try again)", unknown.join(", ")), true)
                }
            }
            Err(e) => (e, true),
        },
        // Handled in the loop, which owns the terminal.
        Effect::EditThenRestart { .. }
        | Effect::View(_)
        | Effect::Doctor
        | Effect::LoadExtensions(_)
        | Effect::ExtInstall { .. } => ("internal: handled by the loop".into(), true),
        Effect::CheckUpdates => {
            let current = devid_daemon::VERSION;
            let devid = match crate::setup::latest_devid(paths) {
                Ok(latest) if devid_daemon::releases::newer(&latest, current) => {
                    format!("devid {latest} available (D to update)")
                }
                Ok(_) => format!("devid {current} is the newest"),
                Err(e) => format!("devid: {e}"),
            };
            let (binaries, failed) = perform(Effect::Update { check: true }, paths);
            (format!("{devid} · {binaries}"), failed)
        }
        Effect::Share(site) => {
            match request(paths, &Request::ShareStart { site: Some(site.clone()), cwd: String::new() }) {
                Ok(v) => match v["url"].as_str() {
                    Some(url) => (format!("{site} is shared at {url} (copy it from the menu's line; h stops)"), false),
                    None => (
                        format!("cloudflared is still connecting for {site}; its link shows in the site's menu"),
                        false,
                    ),
                },
                Err(e) => (e, true),
            }
        }
        Effect::Copy(text) => {
            let copied = Command::new("pbcopy").stdin(Stdio::piped()).spawn().and_then(|mut child| {
                std::io::Write::write_all(child.stdin.as_mut().expect("piped stdin"), text.as_bytes())?;
                child.wait()
            });
            match copied {
                Ok(s) if s.success() => (format!("copied {text}"), false),
                _ => ("could not copy to the clipboard".into(), true),
            }
        }
    }
}

/// Run the TUI. Returns `devid` arguments to run after the TUI closed (a handover), if any.
pub fn run(paths: &Paths) -> Result<Option<Vec<String>>, String> {
    let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
    let mut app = App::new(no_color);
    // init() also installs a panic hook that restores the terminal.
    let mut terminal = ratatui::init();
    let result = (|| -> std::io::Result<Option<Vec<String>>> {
        let mut last: Option<Instant> = None;
        loop {
            let every = if app.follow && app.page == app::Page::Logs { Duration::from_secs(1) } else { REFRESH };
            if last.is_none_or(|t| t.elapsed() >= every) {
                refresh(&mut app, paths);
                last = Some(Instant::now());
            }
            terminal.draw(|frame| ui::draw(frame, &app))?;
            if event::poll(Duration::from_millis(250))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                match app.handle_key(key) {
                    Action::Quit => return Ok(None),
                    Action::Handover(args) => return Ok(Some(args)),
                    Action::Do(Effect::Doctor) => {
                        app.message = Some(match crate::setup::doctor_checks(paths) {
                            Ok(checks) => {
                                let problems = checks.iter().filter(|c| !c.ok).count();
                                app.checks = checks.into_iter().map(|c| (c.ok, c.what, c.fix)).collect();
                                match problems {
                                    0 => ("doctor: all good".to_string(), false),
                                    n => (format!("doctor: {n} problem(s), see below"), true),
                                }
                            }
                            Err(e) => (e, true),
                        });
                    }
                    Action::Refresh => last = None,
                    Action::Do(Effect::LoadExtensions(version)) => {
                        app.message = Some(("⏳ loading the catalog…".into(), false));
                        terminal.draw(|frame| ui::draw(frame, &app))?;
                        app.message = None;
                        load_extensions(&mut app, paths, &version);
                    }
                    Action::Do(Effect::ExtInstall { version, names }) => {
                        // The install runs in another thread; show its progress line until it replies.
                        let reply = crate::ext_install_with_progress(paths, &version, &names, |line| {
                            app.message = Some((format!("⏳ {line}"), false));
                            let _ = terminal.draw(|frame| ui::draw(frame, &app));
                        });
                        app.message = Some(match reply {
                            Ok(v) => crate::ext_install_summary(&v),
                            Err(e) => (e, true),
                        });
                        if let Some(v) = app.reopen.take() {
                            load_extensions(&mut app, paths, &v);
                        }
                        last = None;
                    }
                    Action::Do(Effect::View(path)) => {
                        ratatui::restore();
                        let _ = std::process::Command::new("sh")
                            .args(["-c", "${PAGER:-less} \"$1\"", "devid-view"])
                            .arg(&path)
                            .status();
                        terminal = ratatui::init();
                        last = None;
                    }
                    Action::Do(Effect::EditThenRestart { path, service }) => {
                        // Step aside for the editor, then come back and apply the change.
                        ratatui::restore();
                        let edited = std::process::Command::new("sh")
                            .args(["-c", "${VISUAL:-${EDITOR:-vi}} \"$1\"", "devid-edit"])
                            .arg(&path)
                            .status();
                        terminal = ratatui::init();
                        app.message = Some(match edited {
                            Ok(s) if s.success() => {
                                match request(paths, &Request::Restart { service: service.clone() }) {
                                    Ok(_) => (format!("saved {path}; {service} restarted"), false),
                                    Err(e) => (e, true),
                                }
                            }
                            Ok(s) => (format!("editor exited with {s}; nothing restarted"), true),
                            Err(e) => (format!("could not start the editor: {e}"), true),
                        });
                        last = None;
                    }
                    Action::Do(effect) => {
                        // Downloads take seconds; say so before blocking.
                        if matches!(
                            effect,
                            Effect::Daemon { .. } | Effect::Update { .. } | Effect::CheckUpdates | Effect::Share(_)
                        ) {
                            app.message = Some(("⏳ working…".into(), false));
                            terminal.draw(|frame| ui::draw(frame, &app))?;
                        }
                        app.message = Some(perform(effect, paths));
                        // An action from the Extensions view returns to it, with fresh data.
                        if let Some(v) = app.reopen.take() {
                            load_extensions(&mut app, paths, &v);
                        }
                        last = None; // show the change right away
                    }
                    Action::None => {}
                }
            }
        }
    })();
    ratatui::restore();
    result.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::app::{App, Focus, Page};
    use super::ui::draw;
    use ratatui::{Terminal, backend::TestBackend, style::Color};
    use serde_json::{Value, json};

    fn status() -> Value {
        json!({
            "ok": true, "version": "0.1.0", "server": "nginx", "server_status": "running", "sites": 2,
            "default_php": "8.5", "http_port": 8088, "https_port": 8443, "dns_port": 53535,
            "dns": "listening on 127.0.0.1:53535", "tld": "test", "standard_ports": true,
            "home": "/Users/me/.devid", "ca": "/Users/me/.devid/ca/devid-ca.crt",
            "php": [
                {"version": "8.4", "status": "stopped", "pid": null, "default": false},
                {"version": "8.5", "status": "running", "pid": 123, "default": true}
            ]
        })
    }

    fn sites() -> Vec<Value> {
        vec![
            json!({"name": "erdbpro-saas", "url": "https://erdbpro-saas.test", "path": "/p/erdbpro-saas", "php": "8.5", "php_installed": true, "isolated": false}),
            json!({"name": "legacy", "url": "https://legacy.test", "path": "/p/legacy", "php": "8.3", "php_installed": false, "isolated": true}),
        ]
    }

    fn render(app: &App) -> (String, ratatui::buffer::Buffer) {
        let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
        terminal.draw(|f| draw(f, app)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let text = (0..buf.area.height)
            .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        (text, buf)
    }

    fn app(no_color: bool) -> App {
        let mut app = App::new(no_color);
        app.status = Ok(status());
        app.sites = sites();
        app
    }

    #[test]
    fn header_sidebar_and_sites() {
        let (text, _) = render(&app(false));
        assert!(text.contains("Devid") && text.contains("● running") && text.contains("nginx"));
        assert!(text.contains("PHP 8.5 default") && text.contains("2 sites"));
        for page in ["1 Sites", "2 PHP", "3 Server", "4 Logs", "5 Settings"] {
            assert!(text.contains(page), "sidebar shows {page}");
        }
        assert!(text.contains("https://erdbpro-saas.test"));
        assert!(text.contains("✗ PHP 8.3 not installed"), "missing PHP is marked with symbol + text");
        assert!(text.contains("q quit"), "footer lists keys");
    }

    #[test]
    fn other_pages_and_help() {
        let mut a = app(false);
        a.page = Page::Php;
        let (text, _) = render(&a);
        assert!(text.contains("PHP 8.5") && text.contains("● running") && text.contains("default"));
        assert!(text.contains("○ stopped"));
        a.page = Page::Server;
        a.servers = vec![
            json!({"server": "nginx", "installed": true, "active": true, "status": "running", "paused": false, "version": "1.30.6"}),
        ];
        let text = render(&a).0;
        assert!(text.contains("* nginx") && text.contains("1.30.6"), "{text}");
        assert!(text.contains("80/443"), "standard ports via the pf redirect are explained");
        a.page = Page::Logs;
        a.logs = vec![json!({"name": "php85-fpm.log", "size": 42})];
        a.log_text = "line one\nNOTICE: ready to handle connections\n".into();
        let text = render(&a).0;
        assert!(text.contains("php85-fpm.log") && text.contains("ready to handle connections"), "{text}");
        a.page = Page::Settings;
        let text = render(&a).0;
        assert!(text.contains("*.test") && text.contains("Press d to run the health check"));
        a.checks = vec![
            (true, "daemon answers".into(), String::new()),
            (false, "Valet is stopped".into(), "run `valet stop`".into()),
        ];
        let text = render(&a).0;
        assert!(text.contains("ok  daemon answers") && text.contains("✗   Valet is stopped"), "{text}");
        assert!(text.contains("fix: run `valet stop`"), "every problem shows its fix");
        a.show_help = true;
        assert!(render(&a).0.contains("press any key to close"));
    }

    #[test]
    fn daemon_down_screen() {
        let mut a = App::new(false);
        a.status = Err("daemon is not running".into());
        let (text, _) = render(&a);
        assert!(text.contains("✗ daemon not running"));
        assert!(text.contains("devid setup"), "the screen says how to fix it");
    }

    #[test]
    fn overlays_and_messages() {
        use super::app::{InputFor, Mode, PickFor};
        let mut a = app(false);
        a.focus = Focus::Page;
        a.mode = Mode::Menu { row: 0, items: App::site_menu(&a.sites[0]), index: 0 };
        let (text, _) = render(&a);
        assert!(text.contains("erdbpro-saas") && text.contains("o  Open in browser") && text.contains("c  Copy URL"));
        assert!(text.contains("y  App type") && text.contains("n  Add a command"), "Phase 8 site actions");
        a.mode = Mode::Input { purpose: InputFor::Command("legacy".into()), text: "dev npm run dev".into() };
        assert!(render(&a).0.contains("name command: dev npm run dev"));
        a.mode = Mode::Input { purpose: InputFor::Link, text: "~/code".into() };
        assert!(render(&a).0.contains("path: ~/code"));
        a.mode = Mode::Pick {
            purpose: PickFor::Isolate("legacy".into()),
            options: vec!["8.4".into(), "8.5".into()],
            index: 1,
        };
        assert!(render(&a).0.contains("PHP version for legacy"));
        a.mode = Mode::Confirm {
            question: "Unlink legacy?".into(),
            request: Box::new(devid_daemon::protocol::Request::Unlink { name: "legacy".into() }),
            done: String::new(),
        };
        assert!(render(&a).0.contains("y yes · n no"));
        a.mode = Mode::Normal;
        a.message = Some(("legacy now uses PHP 8.4".into(), false));
        assert!(render(&a).0.contains("✓ legacy now uses PHP 8.4"), "success replaces the key hints");
        a.message = Some(("PHP 8.3 is not installed".into(), true));
        assert!(render(&a).0.contains("✗ PHP 8.3 is not installed"));
        a.message = None;
        assert!(render(&a).0.contains("enter actions"), "Sites keys when the page has focus");
    }

    #[test]
    fn extensions_view_and_its_confirmation() {
        use super::app::Mode;
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut a = app(false);
        a.page = Page::Php;
        a.focus = Focus::Page;
        let catalog = json!({"release": "2026.11.04-6", "extensions": [
            {"name": "apcu", "version": "5.1.28", "license": "PHP-3.01", "size": 33_000, "installed": null},
            {"name": "imagick", "version": "3.8.1", "license": "PHP-3.01", "size": 9_500_000, "installed": "3.8.1"}
        ]});
        let list = json!({"bundled": ["Core", "gd", "intl"], "shipped": {"pcov": false, "xdebug": true}});
        a.open_extensions("8.5", &catalog, &list);
        a.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        let (text, _) = render(&a);
        for want in [
            "PHP 8.5 · Extensions",
            "Catalog (release 2026.11.04-6)",
            "[x] apcu",
            "available  33 KB",
            "✓  imagick",
            "installed 3.8.1",
            "[ Install selected (1) ]",
            "xdebug on",
            "Core, gd, intl",
        ] {
            assert!(text.contains(want), "{want:?} missing from:\n{text}");
        }
        a.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        a.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(a.mode, Mode::ConfirmExt { .. }));
        let (text, _) = render(&a);
        assert!(text.contains("Install apcu 5.1.28 for PHP 8.5?") && text.contains("y yes · n back"), "{text}");
    }

    #[test]
    fn no_color_means_no_color() {
        let mut a = app(true);
        a.focus = Focus::Page;
        let (_, buf) = render(&a);
        for cell in buf.content() {
            assert!(matches!(cell.fg, Color::Reset) && matches!(cell.bg, Color::Reset), "NO_COLOR leaves colors unset");
        }
        let (_, colored) = render(&app(false));
        assert!(colored.content().iter().any(|c| c.fg == Color::Green), "colors are used without NO_COLOR");
    }
}
