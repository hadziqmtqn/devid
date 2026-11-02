//! `devid` with no arguments: the TUI (Phase 4, D-012). It only renders daemon state and sends
//! daemon requests; it runs only while open and polls `status` every 2 s.

mod app;
mod ui;

use crate::request;
use app::{Action, App};
use devid_daemon::{paths::Paths, protocol::Request};
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use std::time::{Duration, Instant};

const REFRESH: Duration = Duration::from_secs(2);

fn refresh(app: &mut App, paths: &Paths) {
    match request(paths, &Request::Status) {
        Ok(status) => {
            app.sites =
                request(paths, &Request::Sites).ok().and_then(|v| v["sites"].as_array().cloned()).unwrap_or_default();
            app.status = Ok(status);
        }
        Err(e) => {
            app.status = Err(e);
            app.sites.clear();
        }
    }
}

pub fn run(paths: &Paths) -> Result<(), String> {
    let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
    let mut app = App::new(no_color);
    // init() also installs a panic hook that restores the terminal.
    let mut terminal = ratatui::init();
    let result = (|| -> std::io::Result<()> {
        let mut last: Option<Instant> = None;
        loop {
            if last.is_none_or(|t| t.elapsed() >= REFRESH) {
                refresh(&mut app, paths);
                last = Some(Instant::now());
            }
            terminal.draw(|frame| ui::draw(frame, &app))?;
            if event::poll(Duration::from_millis(250))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                match app.handle_key(key) {
                    Action::Quit => return Ok(()),
                    Action::Refresh => last = None,
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
        assert!(render(&a).0.contains("80/443"), "standard ports via the pf redirect are explained");
        a.page = Page::Settings;
        assert!(render(&a).0.contains("*.test"));
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
