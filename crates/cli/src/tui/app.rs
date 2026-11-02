//! TUI state and key handling. Pure: no terminal, no daemon, no side effects. Actions come out
//! as `Effect`s that mod.rs carries out, so everything here is tested directly.

use devid_daemon::protocol::Request;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Sites,
    Php,
    Server,
    Logs,
    Settings,
}

impl Page {
    pub const ALL: [Page; 5] = [Page::Sites, Page::Php, Page::Server, Page::Logs, Page::Settings];

    pub fn title(self) -> &'static str {
        match self {
            Page::Sites => "Sites",
            Page::Php => "PHP",
            Page::Server => "Server",
            Page::Logs => "Logs",
            Page::Settings => "Settings",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Page,
}

/// Something the loop must do outside this pure state.
#[derive(Debug)]
pub enum Effect {
    /// A daemon request; `done` is the message shown when it succeeds.
    Daemon {
        request: Request,
        done: String,
    },
    OpenUrl(String),
    OpenTerminal(String),
    Copy(String),
    /// Edit a file in $EDITOR (the TUI steps aside), then restart `service`.
    EditThenRestart {
        path: String,
        service: String,
    },
    /// Check for (or install) newer PHP/nginx/Caddy from the release.
    Update {
        check: bool,
    },
}

#[derive(Debug)]
pub enum Action {
    None,
    Quit,
    Refresh,
    Do(Effect),
}

/// What text input is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFor {
    Link,
    Park,
}

/// What a pick list is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickFor {
    /// Isolate this site to the picked PHP version.
    Isolate(String),
    Unpark,
}

/// An overlay that takes the keys until it closes.
#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Normal,
    /// Actions for one row of the current page (a site or a PHP version): (key, label), selected.
    Menu {
        row: usize,
        items: Vec<(char, String)>,
        index: usize,
    },
    Input {
        purpose: InputFor,
        text: String,
    },
    Pick {
        purpose: PickFor,
        options: Vec<String>,
        index: usize,
    },
    /// A destructive action waiting for y/n.
    Confirm {
        question: String,
        request: Box<Request>,
        done: String,
    },
}

/// Everything the screen shows. `status`/`sites` come from the daemon; `Err` means it is down.
pub struct App {
    pub page: Page,
    pub focus: Focus,
    /// Selected row on the current page.
    pub row: usize,
    pub show_help: bool,
    pub no_color: bool,
    pub mode: Mode,
    /// Feedback for the last action: (text, is_error).
    pub message: Option<(String, bool)>,
    pub status: Result<Value, String>,
    pub sites: Vec<Value>,
    pub parked: Vec<String>,
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

impl App {
    pub fn new(no_color: bool) -> Self {
        Self {
            page: Page::Sites,
            focus: Focus::Sidebar,
            row: 0,
            show_help: false,
            no_color,
            mode: Mode::Normal,
            message: None,
            status: Err("connecting…".into()),
            sites: Vec::new(),
            parked: Vec::new(),
        }
    }

    /// Rows on the current page, for moving the selection.
    pub fn rows(&self) -> usize {
        match self.page {
            Page::Sites => self.sites.len(),
            Page::Php => devid_daemon::php::SUPPORTED.len(),
            Page::Server => 3,
            Page::Logs | Page::Settings => 0,
        }
    }

    /// Installed PHP versions from the last status.
    pub fn php_versions(&self) -> Vec<String> {
        self.status
            .as_ref()
            .ok()
            .and_then(|st| st["php"].as_array())
            .map(|l| l.iter().map(|p| s(&p["version"])).collect())
            .unwrap_or_default()
    }

    /// The installed entry for a supported version, if installed.
    pub fn php_entry(&self, version: &str) -> Option<&Value> {
        self.status.as_ref().ok()?.get("php")?.as_array()?.iter().find(|p| p["version"] == version)
    }

    /// Actions for a PHP version row.
    pub fn php_menu(entry: Option<&Value>, version: &str) -> Vec<(char, String)> {
        let Some(p) = entry else { return vec![('i', format!("Install PHP {version} (download, sha256-checked)"))] };
        let on = |e: &str| p["extensions"][e] == true;
        let mut items = Vec::new();
        if p["default"] != true {
            items.push(('d', "Make default".to_string()));
        }
        items.push(('x', format!("Turn Xdebug {}", if on("xdebug") { "off" } else { "on" })));
        items.push(('c', format!("Turn pcov {}", if on("pcov") { "off" } else { "on" })));
        items.push(('e', "Edit php.ini (restarts php-fpm after)".to_string()));
        items.push(('r', "Restart php-fpm".to_string()));
        items
    }

    fn php_action(&mut self, row: usize, choice: char) -> Action {
        let version = devid_daemon::php::SUPPORTED[row.min(devid_daemon::php::SUPPORTED.len() - 1)].to_string();
        let entry = self.php_entry(&version).cloned();
        let service = format!("php-{version}");
        let daemon = |request, done: String| Action::Do(Effect::Daemon { request, done });
        match (choice, entry) {
            ('i', None) => {
                daemon(Request::PhpInstall { version: version.clone(), from: None }, format!("PHP {version} installed"))
            }
            ('d', Some(_)) => {
                daemon(Request::PhpUse { version: version.clone() }, format!("PHP {version} is now the default"))
            }
            (c @ ('x' | 'c'), Some(p)) => {
                let ext = if c == 'x' { "xdebug" } else { "pcov" };
                let enabled = p["extensions"][ext] != true;
                daemon(
                    Request::PhpExt { version: version.clone(), ext: ext.into(), enabled },
                    format!("{ext} {} for PHP {version}", if enabled { "on" } else { "off" }),
                )
            }
            ('e', Some(p)) => Action::Do(Effect::EditThenRestart { path: s(&p["ini"]), service }),
            ('r', Some(_)) => daemon(Request::Restart { service: service.clone() }, format!("restarted {service}")),
            _ => Action::None,
        }
    }

    pub fn selected_site(&self) -> Option<&Value> {
        (self.page == Page::Sites).then(|| self.sites.get(self.row)).flatten()
    }

    /// The menu entries for a site; unlink only for linked sites, unisolate only when isolated.
    pub fn site_menu(site: &Value) -> Vec<(char, String)> {
        let mut items = vec![
            ('o', "Open in browser".to_string()),
            ('t', "Open folder in Terminal".to_string()),
            ('c', "Copy URL".to_string()),
            ('i', "Isolate PHP version…".to_string()),
        ];
        if site["isolated"] == true {
            items.push(('u', "Use the default PHP again".to_string()));
        }
        if site["linked"] == true {
            items.push(('d', "Unlink…".to_string()));
        }
        items
    }

    fn select_page(&mut self, page: Page) {
        if self.page != page {
            self.page = page;
            self.row = 0;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Action::Quit;
        }
        if self.show_help {
            // Any key closes help; q/esc do not also quit.
            self.show_help = false;
            return Action::None;
        }
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Normal => self.normal_key(key),
            Mode::Menu { row, items, index } => self.menu_key(key, row, items, index),
            Mode::Input { purpose, text } => self.input_key(key, purpose, text),
            Mode::Pick { purpose, options, index } => self.pick_key(key, purpose, options, index),
            Mode::Confirm { question, request, done } => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => Action::Do(Effect::Daemon { request: *request, done }),
                KeyCode::Char('n') | KeyCode::Esc => Action::None,
                _ => {
                    self.mode = Mode::Confirm { question, request, done };
                    Action::None
                }
            },
        }
    }

    fn normal_key(&mut self, key: KeyEvent) -> Action {
        // Clear the previous action's message on the next key.
        self.message = None;
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Action::Quit,
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Char('r') => return Action::Refresh,
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus = if self.focus == Focus::Sidebar { Focus::Page } else { Focus::Sidebar };
            }
            KeyCode::Char(c @ '1'..='5') => self.select_page(Page::ALL[c as usize - '1' as usize]),
            KeyCode::Up | KeyCode::Char('k') => self.step(-1),
            KeyCode::Down | KeyCode::Char('j') => self.step(1),
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') if self.focus == Focus::Sidebar => {
                self.focus = Focus::Page;
            }
            KeyCode::Left | KeyCode::Char('h') if self.focus == Focus::Page => self.focus = Focus::Sidebar,
            // Sites page actions.
            KeyCode::Enter if self.page == Page::Sites && self.focus == Focus::Page => {
                if let Some(site) = self.selected_site() {
                    let items = Self::site_menu(site);
                    self.mode = Mode::Menu { row: self.row, items, index: 0 };
                }
            }
            KeyCode::Enter if self.page == Page::Php && self.focus == Focus::Page => {
                let version = devid_daemon::php::SUPPORTED[self.row.min(devid_daemon::php::SUPPORTED.len() - 1)];
                let items = Self::php_menu(self.php_entry(version), version);
                self.mode = Mode::Menu { row: self.row, items, index: 0 };
            }
            KeyCode::Char('u') if self.page == Page::Php => return Action::Do(Effect::Update { check: true }),
            KeyCode::Char('U') if self.page == Page::Php => {
                self.mode = Mode::Confirm {
                    question: "Download and install newer PHP/nginx/Caddy from Devid's release?".into(),
                    request: Box::new(Request::Update { check: false }),
                    done: "updated".into(),
                };
            }
            KeyCode::Char('a') if self.page == Page::Sites => {
                self.mode = Mode::Input { purpose: InputFor::Link, text: String::new() };
            }
            KeyCode::Char('p') if self.page == Page::Sites => {
                self.mode = Mode::Input { purpose: InputFor::Park, text: String::new() };
            }
            KeyCode::Char('U') if self.page == Page::Sites => {
                if self.parked.is_empty() {
                    self.message = Some(("No parked folders.".into(), true));
                } else {
                    self.mode = Mode::Pick { purpose: PickFor::Unpark, options: self.parked.clone(), index: 0 };
                }
            }
            _ => {}
        }
        Action::None
    }

    fn menu_key(&mut self, key: KeyEvent, row: usize, items: Vec<(char, String)>, index: usize) -> Action {
        let chosen = match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return Action::None,
            KeyCode::Up | KeyCode::Char('k') => {
                self.mode = Mode::Menu { row, index: index.saturating_sub(1), items };
                return Action::None;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.mode = Mode::Menu { row, index: (index + 1).min(items.len() - 1), items };
                return Action::None;
            }
            KeyCode::Enter => items.get(index).map(|(k, _)| *k),
            KeyCode::Char(c) => items.iter().find(|(k, _)| *k == c).map(|(k, _)| *k),
            _ => None,
        };
        let Some(choice) = chosen else {
            self.mode = Mode::Menu { row, items, index };
            return Action::None;
        };
        if self.page == Page::Php {
            return self.php_action(row, choice);
        }
        let Some(site) = self.sites.get(row).cloned() else { return Action::None };
        let (name, url, path) = (s(&site["name"]), s(&site["url"]), s(&site["path"]));
        match choice {
            'o' => Action::Do(Effect::OpenUrl(url)),
            't' => Action::Do(Effect::OpenTerminal(path)),
            'c' => Action::Do(Effect::Copy(url)),
            'i' => {
                let options = self.php_versions();
                if options.is_empty() {
                    self.message = Some(("No PHP installed; install one on the PHP page.".into(), true));
                } else {
                    self.mode = Mode::Pick { purpose: PickFor::Isolate(name), options, index: 0 };
                }
                Action::None
            }
            'u' => Action::Do(Effect::Daemon {
                request: Request::Unisolate { site: Some(name.clone()), cwd: String::new() },
                done: format!("{name} uses the default PHP again"),
            }),
            'd' => {
                self.mode = Mode::Confirm {
                    question: format!("Unlink {name}? The folder itself is not touched."),
                    request: Box::new(Request::Unlink { name: name.clone() }),
                    done: format!("unlinked {name}"),
                };
                Action::None
            }
            _ => Action::None,
        }
    }

    fn input_key(&mut self, key: KeyEvent, purpose: InputFor, mut text: String) -> Action {
        match key.code {
            KeyCode::Esc => return Action::None,
            KeyCode::Enter => {
                let path = expand_tilde(text.trim());
                if path.is_empty() {
                    return Action::None;
                }
                let (request, done) = match purpose {
                    InputFor::Link => (Request::Link { name: None, path: path.clone() }, format!("linked {path}")),
                    InputFor::Park => (Request::Park { path: path.clone() }, format!("parked {path}")),
                };
                return Action::Do(Effect::Daemon { request, done });
            }
            KeyCode::Backspace => {
                text.pop();
            }
            KeyCode::Char(c) => text.push(c),
            _ => {}
        }
        self.mode = Mode::Input { purpose, text };
        Action::None
    }

    fn pick_key(&mut self, key: KeyEvent, purpose: PickFor, options: Vec<String>, index: usize) -> Action {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => Action::None,
            KeyCode::Up | KeyCode::Char('k') => {
                self.mode = Mode::Pick { purpose, options, index: index.saturating_sub(1) };
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let index = (index + 1).min(options.len().saturating_sub(1));
                self.mode = Mode::Pick { purpose, options, index };
                Action::None
            }
            KeyCode::Enter => {
                let Some(choice) = options.get(index).cloned() else { return Action::None };
                Action::Do(match purpose {
                    PickFor::Isolate(site) => Effect::Daemon {
                        request: Request::Isolate {
                            version: choice.clone(),
                            site: Some(site.clone()),
                            cwd: String::new(),
                        },
                        done: format!("{site} now uses PHP {choice}"),
                    },
                    PickFor::Unpark => Effect::Daemon {
                        request: Request::Unpark { path: choice.clone() },
                        done: format!("unparked {choice}"),
                    },
                })
            }
            _ => {
                self.mode = Mode::Pick { purpose, options, index };
                Action::None
            }
        }
    }

    fn step(&mut self, delta: isize) {
        match self.focus {
            Focus::Sidebar => {
                let i = Page::ALL.iter().position(|p| *p == self.page).unwrap_or(0) as isize;
                let next = (i + delta).clamp(0, Page::ALL.len() as isize - 1) as usize;
                self.select_page(Page::ALL[next]);
            }
            Focus::Page => {
                let rows = self.rows();
                if rows > 0 {
                    self.row = (self.row as isize + delta).clamp(0, rows as isize - 1) as usize;
                }
            }
        }
    }
}

/// `~/x` -> `$HOME/x`; the daemon needs absolute paths (it also checks they exist).
pub fn expand_tilde(path: &str) -> String {
    match (path.strip_prefix('~'), std::env::var("HOME")) {
        (Some(rest), Ok(home)) if rest.is_empty() || rest.starts_with('/') => format!("{home}{rest}"),
        _ => path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyEvent, KeyModifiers};
    use serde_json::json;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ch(c: char) -> KeyEvent {
        key(KeyCode::Char(c))
    }

    fn with_sites() -> App {
        let mut app = App::new(false);
        app.status = Ok(json!({"php": [{"version": "8.4"}, {"version": "8.5"}]}));
        app.sites = vec![
            json!({"name": "blog", "url": "https://blog.test", "path": "/p/blog", "linked": false, "isolated": false}),
            json!({"name": "shop", "url": "https://shop.test", "path": "/x/shop", "linked": true, "isolated": true}),
        ];
        app.parked = vec!["/p".into()];
        app.focus = Focus::Page;
        app
    }

    fn daemon_json(action: Action) -> Value {
        match action {
            Action::Do(Effect::Daemon { request, .. }) => serde_json::to_value(request).unwrap(),
            other => panic!("expected a daemon request, got {other:?}"),
        }
    }

    #[test]
    fn navigation() {
        let mut app = App::new(false);
        assert_eq!(app.page, Page::Sites);
        app.handle_key(key(KeyCode::Down));
        app.handle_key(ch('j'));
        assert_eq!(app.page, Page::Server, "sidebar moves between pages");
        app.handle_key(ch('1'));
        assert_eq!(app.page, Page::Sites, "number keys jump to a page");
        app.handle_key(key(KeyCode::Up));
        assert_eq!(app.page, Page::Sites, "stays at the top");

        app.sites = vec![json!({"name": "a"}), json!({"name": "b"})];
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.focus, Focus::Page);
        for _ in 0..5 {
            app.handle_key(key(KeyCode::Down));
        }
        assert_eq!(app.row, 1, "row selection is clamped to the list");
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.focus, Focus::Sidebar);
        app.handle_key(ch('2'));
        assert_eq!((app.page, app.row), (Page::Php, 0), "changing page resets the row");
    }

    #[test]
    fn help_quit_refresh() {
        let mut app = App::new(false);
        assert!(matches!(app.handle_key(ch('?')), Action::None));
        assert!(app.show_help);
        assert!(matches!(app.handle_key(ch('q')), Action::None), "q closes help first");
        assert!(!app.show_help);
        assert!(matches!(app.handle_key(ch('r')), Action::Refresh));
        assert!(matches!(app.handle_key(ch('q')), Action::Quit));
        assert!(matches!(app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)), Action::Quit));
    }

    #[test]
    fn site_menu_entries_depend_on_the_site() {
        let app = with_sites();
        let keys = |site: &Value| App::site_menu(site).iter().map(|(k, _)| *k).collect::<String>();
        assert_eq!(keys(&app.sites[0]), "otci", "parked, default PHP: no unisolate, no unlink");
        assert_eq!(keys(&app.sites[1]), "otciud", "linked + isolated");
    }

    #[test]
    fn open_terminal_and_copy() {
        let mut app = with_sites();
        app.handle_key(key(KeyCode::Enter));
        assert!(matches!(app.mode, Mode::Menu { .. }));
        assert!(matches!(app.handle_key(ch('o')), Action::Do(Effect::OpenUrl(u)) if u == "https://blog.test"));
        assert_eq!(app.mode, Mode::Normal, "the menu closes after an action");
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Down));
        assert!(matches!(app.handle_key(key(KeyCode::Enter)), Action::Do(Effect::OpenTerminal(p)) if p == "/p/blog"));
        app.handle_key(key(KeyCode::Enter));
        assert!(matches!(app.handle_key(ch('c')), Action::Do(Effect::Copy(u)) if u == "https://blog.test"));
        app.handle_key(key(KeyCode::Enter));
        assert!(matches!(app.handle_key(key(KeyCode::Esc)), Action::None));
        assert_eq!(app.mode, Mode::Normal, "esc closes the menu");
    }

    #[test]
    fn isolate_unisolate_and_unlink() {
        let mut app = with_sites();
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(ch('i'));
        assert!(matches!(&app.mode, Mode::Pick { options, .. } if options == &["8.4", "8.5"]));
        app.handle_key(key(KeyCode::Down));
        let req = daemon_json(app.handle_key(key(KeyCode::Enter)));
        assert_eq!(req, json!({"cmd": "isolate", "version": "8.5", "site": "blog", "cwd": ""}));

        app.row = 1; // shop: linked + isolated
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(daemon_json(app.handle_key(ch('u'))), json!({"cmd": "unisolate", "site": "shop", "cwd": ""}));

        app.handle_key(key(KeyCode::Enter));
        assert!(matches!(app.handle_key(ch('d')), Action::None));
        assert!(matches!(&app.mode, Mode::Confirm { question, .. } if question.contains("Unlink shop")));
        assert!(matches!(app.handle_key(ch('x')), Action::None));
        assert!(matches!(app.mode, Mode::Confirm { .. }), "other keys keep asking");
        assert!(matches!(app.handle_key(ch('n')), Action::None));
        assert_eq!(app.mode, Mode::Normal, "n cancels");
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(ch('d'));
        assert_eq!(daemon_json(app.handle_key(ch('y'))), json!({"cmd": "unlink", "name": "shop"}));
    }

    #[test]
    fn link_park_unpark() {
        let mut app = with_sites();
        app.handle_key(ch('a'));
        for c in "~/code/app".chars() {
            app.handle_key(ch(c));
        }
        app.handle_key(key(KeyCode::Backspace));
        app.handle_key(ch('p'));
        let home = std::env::var("HOME").unwrap();
        assert_eq!(
            daemon_json(app.handle_key(key(KeyCode::Enter))),
            json!({"cmd": "link", "name": null, "path": format!("{home}/code/app")}),
            "~ is expanded and backspace edits"
        );
        app.handle_key(ch('p'));
        app.handle_key(ch('/'));
        app.handle_key(ch('w'));
        assert_eq!(daemon_json(app.handle_key(key(KeyCode::Enter))), json!({"cmd": "park", "path": "/w"}));
        app.handle_key(ch('a'));
        assert!(matches!(app.handle_key(key(KeyCode::Esc)), Action::None));
        assert_eq!(app.mode, Mode::Normal, "esc cancels input");

        app.handle_key(ch('U'));
        assert_eq!(daemon_json(app.handle_key(key(KeyCode::Enter))), json!({"cmd": "unpark", "path": "/p"}));
        app.parked.clear();
        app.handle_key(ch('U'));
        assert_eq!(app.message.as_ref().map(|m| m.1), Some(true), "nothing to unpark is reported");
    }

    #[test]
    fn php_page_actions() {
        let mut app = App::new(false);
        app.status = Ok(json!({"php": [
            {"version": "8.4", "default": false, "extensions": {"xdebug": false, "pcov": false}, "ini": "/h/php/8.4/etc/php.ini"},
            {"version": "8.5", "default": true, "extensions": {"xdebug": true, "pcov": false}, "ini": "/h/php/8.5/etc/php.ini"}
        ]}));
        app.page = Page::Php;
        app.focus = Focus::Page;
        assert_eq!(app.rows(), 3, "8.3, 8.4, 8.5 (D-009)");

        // 8.3 is not installed: the only action is install.
        app.handle_key(key(KeyCode::Enter));
        assert!(matches!(&app.mode, Mode::Menu { items, .. } if items.len() == 1 && items[0].0 == 'i'));
        assert_eq!(daemon_json(app.handle_key(ch('i'))), json!({"cmd": "php_install", "version": "8.3", "from": null}));

        // 8.4: make default, xdebug on.
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(daemon_json(app.handle_key(ch('d'))), json!({"cmd": "php_use", "version": "8.4"}));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(
            daemon_json(app.handle_key(ch('x'))),
            json!({"cmd": "php_ext", "version": "8.4", "ext": "xdebug", "enabled": true})
        );

        // 8.5 is the default (no "make default"); xdebug is on, so the menu turns it off.
        app.handle_key(key(KeyCode::Down));
        let items = App::php_menu(app.php_entry("8.5"), "8.5");
        assert!(!items.iter().any(|(k, _)| *k == 'd'));
        assert!(items.iter().any(|(_, l)| l == "Turn Xdebug off"));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(
            daemon_json(app.handle_key(ch('x'))),
            json!({"cmd": "php_ext", "version": "8.5", "ext": "xdebug", "enabled": false})
        );
        app.handle_key(key(KeyCode::Enter));
        assert!(matches!(
            app.handle_key(ch('e')),
            Action::Do(Effect::EditThenRestart { path, service }) if path == "/h/php/8.5/etc/php.ini" && service == "php-8.5"
        ));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(daemon_json(app.handle_key(ch('r'))), json!({"cmd": "restart", "service": "php-8.5"}));

        // Updates: u checks, U asks first.
        assert!(matches!(app.handle_key(ch('u')), Action::Do(Effect::Update { check: true })));
        app.handle_key(ch('U'));
        assert!(matches!(app.mode, Mode::Confirm { .. }));
        assert_eq!(daemon_json(app.handle_key(ch('y'))), json!({"cmd": "update", "check": false}));
    }

    #[test]
    fn tilde() {
        let home = std::env::var("HOME").unwrap();
        assert_eq!(expand_tilde("~"), home);
        assert_eq!(expand_tilde("~/a"), format!("{home}/a"));
        assert_eq!(expand_tilde("~other/a"), "~other/a");
        assert_eq!(expand_tilde("/abs"), "/abs");
    }
}
