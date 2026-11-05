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
    /// Show a file read-only in $PAGER (the TUI steps aside).
    View(String),
    /// Run `devid doctor`'s checks and show them on the Settings page.
    Doctor,
    /// Check for a newer devid and newer PHP/nginx/Caddy.
    CheckUpdates,
    /// Load a PHP version's catalog and extension list, then show the Extensions view.
    LoadExtensions(String),
    /// Install catalog extensions with a progress line, then show the Extensions view again.
    ExtInstall {
        version: String,
        names: Vec<String>,
    },
}

/// One catalog extension in the Extensions view.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtRow {
    pub name: String,
    pub version: String,
    pub license: String,
    pub size: u64,
    /// The installed version, if installed.
    pub installed: Option<String>,
}

/// One PHP version's extensions (Phase 6 E4): catalog rows with checkboxes, then the Install button
/// (selected when `index == rows.len()`), Xdebug/pcov and the bundled list.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtView {
    pub version: String,
    pub release: String,
    pub rows: Vec<ExtRow>,
    pub bundled: Vec<String>,
    /// xdebug, pcov: on or off.
    pub shipped: Vec<(String, bool)>,
    /// Names ticked for install.
    pub selected: Vec<String>,
    pub index: usize,
}

/// What a confirmation in the Extensions view does.
#[derive(Debug, Clone, PartialEq)]
pub enum ExtChange {
    Install(Vec<String>),
    Remove(String),
}

/// "33 KB", "2.9 MB".
pub fn human_size(bytes: u64) -> String {
    match bytes {
        b if b >= 1 << 20 => format!("{:.1} MB", b as f64 / (1 << 20) as f64),
        b => format!("{} KB", b.div_ceil(1024)),
    }
}

#[derive(Debug)]
pub enum Action {
    None,
    Quit,
    Refresh,
    Do(Effect),
    /// Close the TUI and run `devid <args>` in the plain terminal (sudo prompts, daemon restarts).
    Handover(Vec<String>),
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
    /// Like Confirm, but yes closes the TUI and runs `devid <args>`.
    ConfirmHandover {
        question: String,
        args: Vec<String>,
    },
    Extensions(ExtView),
    /// An install or remove from the Extensions view waiting for y/n; no goes back to the view.
    ConfirmExt {
        question: String,
        version: String,
        change: ExtChange,
        back: Box<ExtView>,
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
    /// `server_list` entries (nginx, caddy, apache).
    pub servers: Vec<Value>,
    /// Log files (name, size) and the selected log's text.
    pub logs: Vec<Value>,
    pub log_text: String,
    /// Logs page: refresh every second.
    pub follow: bool,
    /// Last `doctor` results on the Settings page: (ok, what, fix).
    pub checks: Vec<(bool, String, String)>,
    /// After this action, load the Extensions view of this PHP version again.
    pub reopen: Option<String>,
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
            servers: Vec::new(),
            logs: Vec::new(),
            log_text: String::new(),
            follow: false,
            checks: Vec::new(),
            reopen: None,
        }
    }

    /// Show the Extensions view from `php_ext_catalog` and `php_ext_list` replies.
    pub fn open_extensions(&mut self, version: &str, catalog: &Value, list: &Value) {
        let strings = |v: &Value| -> Vec<String> { v.as_array().into_iter().flatten().map(s).collect() };
        let rows = catalog["extensions"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|e| ExtRow {
                name: s(&e["name"]),
                version: s(&e["version"]),
                license: s(&e["license"]),
                size: e["size"].as_u64().unwrap_or(0),
                installed: e["installed"].as_str().map(str::to_string),
            })
            .collect();
        let shipped =
            list["shipped"].as_object().into_iter().flatten().map(|(k, on)| (k.clone(), on == true)).collect();
        self.mode = Mode::Extensions(ExtView {
            version: version.to_string(),
            release: s(&catalog["release"]),
            rows,
            bundled: strings(&list["bundled"]),
            shipped,
            selected: Vec::new(),
            index: 0,
        });
    }

    /// Rows on the current page, for moving the selection.
    pub fn rows(&self) -> usize {
        match self.page {
            Page::Sites => self.sites.len(),
            Page::Php => devid_daemon::php::SUPPORTED.len(),
            Page::Server => self.servers.len(),
            Page::Logs => self.logs.len(),
            Page::Settings => 0,
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
        items.push(('e', "Extensions… (install from the catalog)".to_string()));
        items.push(('x', format!("Turn Xdebug {}", if on("xdebug") { "off" } else { "on" })));
        items.push(('c', format!("Turn pcov {}", if on("pcov") { "off" } else { "on" })));
        items.push(('i', "Edit php.ini (restarts php-fpm after)".to_string()));
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
            ('e', Some(_)) => Action::Do(Effect::LoadExtensions(version)),
            ('i', Some(p)) => Action::Do(Effect::EditThenRestart { path: s(&p["ini"]), service }),
            ('r', Some(_)) => daemon(Request::Restart { service: service.clone() }, format!("restarted {service}")),
            _ => Action::None,
        }
    }

    /// The selected log's name, when on the Logs page.
    pub fn selected_log(&self) -> Option<String> {
        (self.page == Page::Logs).then(|| self.logs.get(self.row)).flatten().map(|l| s(&l["name"]))
    }

    /// Actions for a web server row.
    pub fn server_menu(server: &Value) -> Vec<(char, String)> {
        let name = s(&server["server"]);
        if server["installed"] != true {
            let from = if name == "apache" { "macOS system httpd" } else { "download from the release" };
            return vec![('i', format!("Install {name} ({from})"))];
        }
        if server["active"] != true {
            return vec![('u', format!("Use {name} (switches the web server)"))];
        }
        let stopped = server["paused"] == true;
        vec![
            ('r', format!("Restart {name}")),
            ('s', if stopped { format!("Start {name}") } else { format!("Stop {name}") }),
            ('v', "View the generated config".to_string()),
        ]
    }

    fn server_action(&mut self, row: usize, choice: char) -> Action {
        let Some(server) = self.servers.get(row).cloned() else { return Action::None };
        let name = s(&server["server"]);
        let daemon = |request, done: String| Action::Do(Effect::Daemon { request, done });
        match choice {
            'i' => daemon(Request::ServerInstall { server: name.clone(), from: None }, format!("{name} installed")),
            'u' => daemon(Request::ServerUse { server: name.clone() }, format!("now serving with {name}")),
            'r' => daemon(Request::Restart { service: name.clone() }, format!("restarted {name}")),
            's' if server["paused"] == true => {
                daemon(Request::ServiceStart { service: name.clone() }, format!("started {name}"))
            }
            's' => daemon(
                Request::ServiceStop { service: name.clone() },
                format!("stopped {name} (sites are down until you start it)"),
            ),
            'v' => Action::Do(Effect::View(s(&server["config"]))),
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
            Mode::ConfirmHandover { question, args } => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => Action::Handover(args),
                KeyCode::Char('n') | KeyCode::Esc => Action::None,
                _ => {
                    self.mode = Mode::ConfirmHandover { question, args };
                    Action::None
                }
            },
            Mode::Extensions(view) => self.extensions_key(key, view),
            Mode::ConfirmExt { question, version, change, back } => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    self.reopen = Some(version.clone());
                    Action::Do(match change {
                        ExtChange::Install(names) => Effect::ExtInstall { version, names },
                        ExtChange::Remove(name) => Effect::Daemon {
                            request: Request::PhpExtRemove { version: version.clone(), name: name.clone() },
                            done: format!("{name} removed from PHP {version} (php-fpm restarted)"),
                        },
                    })
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.mode = Mode::Extensions(*back);
                    Action::None
                }
                _ => {
                    self.mode = Mode::ConfirmExt { question, version, change, back };
                    Action::None
                }
            },
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
            KeyCode::Enter if self.page == Page::Server && self.focus == Focus::Page => {
                if let Some(server) = self.servers.get(self.row) {
                    let items = Self::server_menu(server);
                    self.mode = Mode::Menu { row: self.row, items, index: 0 };
                }
            }
            KeyCode::Char('S') if self.page == Page::Server => {
                self.mode = Mode::Confirm {
                    question: "Stop the web server and every php-fpm? Sites are down until you start them.".into(),
                    request: Box::new(Request::StopAll),
                    done: "stopped all services".into(),
                };
            }
            KeyCode::Char('A') if self.page == Page::Server => {
                return Action::Do(Effect::Daemon { request: Request::StartAll, done: "started all services".into() });
            }
            KeyCode::Char('d') if self.page == Page::Settings => return Action::Do(Effect::Doctor),
            KeyCode::Char('u') if self.page == Page::Settings => return Action::Do(Effect::CheckUpdates),
            KeyCode::Char('U') if self.page == Page::Settings => {
                self.mode = Mode::Confirm {
                    question: "Download and install newer PHP/nginx/Caddy from Devid's release?".into(),
                    request: Box::new(Request::Update { check: false }),
                    done: "updated".into(),
                };
            }
            KeyCode::Char('D') if self.page == Page::Settings => {
                self.mode = Mode::ConfirmHandover {
                    question: "Update devid itself? The TUI closes, `devid self-update` runs and restarts the daemon."
                        .into(),
                    args: vec!["self-update".into()],
                };
            }
            KeyCode::Char('s') if self.page == Page::Settings => return Action::Handover(vec!["setup".into()]),
            KeyCode::Char('X') if self.page == Page::Settings => {
                self.mode = Mode::ConfirmHandover {
                    question: "Uninstall Devid? The TUI closes; sites stop and the system setup is undone (sudo). ~/.devid data is kept.".into(),
                    args: vec!["uninstall".into()],
                };
            }
            KeyCode::Char('f') if self.page == Page::Logs => {
                self.follow = !self.follow;
                self.message =
                    Some((format!("follow {}", if self.follow { "on (every second)" } else { "off" }), false));
            }
            KeyCode::Char('c') if self.page == Page::Logs => {
                if let Some(name) = self.selected_log() {
                    self.mode = Mode::Confirm {
                        question: format!("Clear {name}? Its contents are deleted."),
                        request: Box::new(Request::LogClear { name: name.clone() }),
                        done: format!("cleared {name}"),
                    };
                }
            }
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
        if self.page == Page::Server {
            return self.server_action(row, choice);
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

    /// Keys in the Extensions view: space ticks a catalog row, enter on the button installs the ticked
    /// ones (after a confirmation), r removes the installed row (after a confirmation).
    fn extensions_key(&mut self, key: KeyEvent, mut v: ExtView) -> Action {
        self.message = None;
        let n = v.rows.len();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return Action::None,
            KeyCode::Up | KeyCode::Char('k') => v.index = v.index.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => v.index = (v.index + 1).min(n),
            KeyCode::Tab | KeyCode::BackTab => v.index = if v.index == n { 0 } else { n },
            KeyCode::Char(' ') | KeyCode::Enter if v.index < n => {
                let row = &v.rows[v.index];
                if let Some(have) = &row.installed {
                    self.message = Some((format!("{} {have} is installed; r removes it", row.name), false));
                } else if let Some(i) = v.selected.iter().position(|s| *s == row.name) {
                    v.selected.remove(i);
                } else {
                    v.selected.push(row.name.clone());
                }
            }
            KeyCode::Enter if v.selected.is_empty() => {
                self.message = Some(("tick extensions with space first".into(), true));
            }
            KeyCode::Enter => {
                // In catalog order, whatever order they were ticked in.
                let picked: Vec<&ExtRow> = v.rows.iter().filter(|r| v.selected.contains(&r.name)).collect();
                let names: Vec<String> = picked.iter().map(|r| r.name.clone()).collect();
                let list: Vec<String> = picked.iter().map(|r| format!("{} {}", r.name, r.version)).collect();
                let size = human_size(picked.iter().map(|r| r.size).sum());
                let question = format!(
                    "Install {} for PHP {}? About {size} from Devid's releases (sha256-checked); php-fpm {} restarts once, so its sites pause for a second or two.",
                    list.join(", "),
                    v.version,
                    v.version
                );
                let version = v.version.clone();
                self.mode =
                    Mode::ConfirmExt { question, version, change: ExtChange::Install(names), back: Box::new(v) };
                return Action::None;
            }
            KeyCode::Char('r') if v.index < n => {
                let row = v.rows[v.index].clone();
                match &row.installed {
                    Some(have) => {
                        let question = format!(
                            "Remove {} {have} from PHP {}? php-fpm {} restarts; code that uses it stops working.",
                            row.name, v.version, v.version
                        );
                        let version = v.version.clone();
                        self.mode = Mode::ConfirmExt {
                            question,
                            version,
                            change: ExtChange::Remove(row.name),
                            back: Box::new(v),
                        };
                        return Action::None;
                    }
                    None => self.message = Some((format!("{} is not installed", row.name), true)),
                }
            }
            KeyCode::Char(c @ ('x' | 'c')) => {
                let ext = if c == 'x' { "xdebug" } else { "pcov" };
                let enabled = !v.shipped.iter().any(|(k, on)| k == ext && *on);
                self.reopen = Some(v.version.clone());
                return Action::Do(Effect::Daemon {
                    request: Request::PhpExt { version: v.version.clone(), ext: ext.into(), enabled },
                    done: format!("{ext} {} for PHP {}", if enabled { "on" } else { "off" }, v.version),
                });
            }
            _ => {}
        }
        self.mode = Mode::Extensions(v);
        Action::None
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
            app.handle_key(ch('i')),
            Action::Do(Effect::EditThenRestart { path, service }) if path == "/h/php/8.5/etc/php.ini" && service == "php-8.5"
        ));
        app.handle_key(key(KeyCode::Enter));
        assert!(matches!(app.handle_key(ch('e')), Action::Do(Effect::LoadExtensions(v)) if v == "8.5"));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(daemon_json(app.handle_key(ch('r'))), json!({"cmd": "restart", "service": "php-8.5"}));

        // Updates: u checks, U asks first.
        assert!(matches!(app.handle_key(ch('u')), Action::Do(Effect::Update { check: true })));
        app.handle_key(ch('U'));
        assert!(matches!(app.mode, Mode::Confirm { .. }));
        assert_eq!(daemon_json(app.handle_key(ch('y'))), json!({"cmd": "update", "check": false}));
    }

    fn ext_app() -> App {
        let mut app = App::new(false);
        app.page = Page::Php;
        app.focus = Focus::Page;
        let catalog = json!({"release": "r1", "extensions": [
            {"name": "apcu", "version": "5.1.28", "license": "PHP-3.01", "size": 33_000, "installed": null},
            {"name": "imagick", "version": "3.8.1", "license": "PHP-3.01", "size": 9_500_000, "installed": "3.8.1"},
            {"name": "mongodb", "version": "2.5.5", "license": "Apache-2.0", "size": 3_000_000, "installed": null}
        ]});
        let list = json!({"bundled": ["Core", "gd"], "shipped": {"pcov": false, "xdebug": true}});
        app.open_extensions("8.5", &catalog, &list);
        app
    }

    fn view(app: &App) -> &ExtView {
        match &app.mode {
            Mode::Extensions(v) => v,
            other => panic!("expected the Extensions view, got {other:?}"),
        }
    }

    #[test]
    fn extensions_view_ticks_installs_and_removes() {
        let mut app = ext_app();
        let v = view(&app);
        assert_eq!((v.rows.len(), v.release.as_str(), v.index), (3, "r1", 0));
        assert_eq!(v.shipped, [("pcov".to_string(), false), ("xdebug".to_string(), true)]);

        // Enter on the button with nothing ticked explains what to do.
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(view(&app).index, 3, "tab jumps to the button");
        app.handle_key(key(KeyCode::Enter));
        assert!(app.message.as_ref().is_some_and(|(m, err)| m.contains("tick") && *err));

        // Tick mongodb, then apcu; an installed row cannot be ticked.
        app.handle_key(key(KeyCode::Tab));
        app.handle_key(ch('j'));
        app.handle_key(ch(' '));
        assert!(app.message.as_ref().is_some_and(|(m, _)| m.contains("imagick 3.8.1 is installed")));
        assert!(view(&app).selected.is_empty());
        app.handle_key(ch('j'));
        app.handle_key(ch(' '));
        app.handle_key(ch('k'));
        app.handle_key(ch('k'));
        app.handle_key(key(KeyCode::Enter)); // enter on a row ticks it too
        app.handle_key(ch(' '));
        app.handle_key(ch(' ')); // untick and tick again
        assert_eq!(view(&app).selected, ["mongodb", "apcu"], "ticked in this order");

        // The button asks first, in catalog order, with the download size; no goes back.
        for _ in 0..3 {
            app.handle_key(ch('j'));
        }
        assert_eq!(view(&app).index, 3);
        app.handle_key(key(KeyCode::Enter));
        let Mode::ConfirmExt { question, change, .. } = &app.mode else { panic!("expected a confirmation") };
        assert!(question.contains("Install apcu 5.1.28, mongodb 2.5.5 for PHP 8.5? About 2.9 MB"), "{question}");
        assert_eq!(change, &ExtChange::Install(vec!["apcu".into(), "mongodb".into()]));
        app.handle_key(ch('n'));
        assert_eq!(view(&app).selected, ["mongodb", "apcu"], "back to the view, ticks kept");
        app.handle_key(key(KeyCode::Enter));
        assert!(matches!(
            app.handle_key(ch('y')),
            Action::Do(Effect::ExtInstall { version, names }) if version == "8.5" && names == ["apcu", "mongodb"]
        ));
        assert_eq!(app.reopen.as_deref(), Some("8.5"), "the view comes back after the install");

        // Remove is one at a time, after its own confirmation; only for an installed row.
        let mut app = ext_app();
        app.handle_key(ch('r'));
        assert!(app.message.as_ref().is_some_and(|(m, err)| m.contains("apcu is not installed") && *err));
        app.handle_key(ch('j'));
        app.handle_key(ch('r'));
        assert!(matches!(&app.mode, Mode::ConfirmExt { change: ExtChange::Remove(n), .. } if n == "imagick"));
        assert_eq!(
            daemon_json(app.handle_key(ch('y'))),
            json!({"cmd": "php_ext_remove", "version": "8.5", "name": "imagick"})
        );

        // Xdebug and pcov switch from here too; esc closes.
        let mut app = ext_app();
        assert_eq!(
            daemon_json(app.handle_key(ch('x'))),
            json!({"cmd": "php_ext", "version": "8.5", "ext": "xdebug", "enabled": false})
        );
        let mut app = ext_app();
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!((human_size(33_000), human_size(9_500_000)), ("33 KB".to_string(), "9.1 MB".to_string()));
    }

    #[test]
    fn server_and_logs_pages() {
        let mut app = App::new(false);
        app.servers = vec![
            json!({"server": "nginx", "installed": true, "active": true, "paused": false, "config": "/h/nginx/conf/nginx.conf"}),
            json!({"server": "caddy", "installed": false, "active": false, "paused": false}),
            json!({"server": "apache", "installed": true, "active": false, "paused": false}),
        ];
        app.page = Page::Server;
        app.focus = Focus::Page;
        let keys = |v: &Value| App::server_menu(v).iter().map(|(k, _)| *k).collect::<String>();
        assert_eq!(keys(&app.servers[0]), "rsv", "active: restart, stop, view");
        assert_eq!(keys(&app.servers[1]), "i");
        assert_eq!(keys(&app.servers[2]), "u");

        app.handle_key(key(KeyCode::Enter));
        assert_eq!(daemon_json(app.handle_key(ch('s'))), json!({"cmd": "service_stop", "service": "nginx"}));
        app.servers[0]["paused"] = json!(true);
        assert!(App::server_menu(&app.servers[0]).iter().any(|(_, l)| l == "Start nginx"));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(daemon_json(app.handle_key(ch('s'))), json!({"cmd": "service_start", "service": "nginx"}));
        app.handle_key(key(KeyCode::Enter));
        assert!(matches!(app.handle_key(ch('v')), Action::Do(Effect::View(p)) if p == "/h/nginx/conf/nginx.conf"));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(
            daemon_json(app.handle_key(ch('i'))),
            json!({"cmd": "server_install", "server": "caddy", "from": null})
        );
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(daemon_json(app.handle_key(ch('u'))), json!({"cmd": "server_use", "server": "apache"}));
        app.handle_key(ch('S'));
        assert!(matches!(app.mode, Mode::Confirm { .. }), "stop all asks first");
        assert_eq!(daemon_json(app.handle_key(ch('y'))), json!({"cmd": "stop_all"}));
        assert_eq!(daemon_json(app.handle_key(ch('A'))), json!({"cmd": "start_all"}));

        app.page = Page::Logs;
        app.row = 0;
        app.logs = vec![json!({"name": "daemon.log", "size": 10}), json!({"name": "php85-fpm.log", "size": 99})];
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.selected_log().as_deref(), Some("php85-fpm.log"));
        app.handle_key(ch('f'));
        assert!(app.follow);
        app.handle_key(ch('c'));
        assert_eq!(daemon_json(app.handle_key(ch('y'))), json!({"cmd": "log_clear", "name": "php85-fpm.log"}));
    }

    #[test]
    fn settings_page() {
        let mut app = App::new(false);
        app.page = Page::Settings;
        assert!(matches!(app.handle_key(ch('d')), Action::Do(Effect::Doctor)));
        assert!(matches!(app.handle_key(ch('u')), Action::Do(Effect::CheckUpdates)));
        assert!(matches!(app.handle_key(ch('s')), Action::Handover(a) if a == ["setup"]));
        app.handle_key(ch('D'));
        assert!(matches!(&app.mode, Mode::ConfirmHandover { args, .. } if args == &["self-update"]));
        assert!(matches!(app.handle_key(ch('y')), Action::Handover(a) if a == ["self-update"]));
        app.handle_key(ch('X'));
        assert!(matches!(app.handle_key(ch('z')), Action::None));
        assert!(matches!(app.mode, Mode::ConfirmHandover { .. }), "other keys keep asking");
        assert!(matches!(app.handle_key(ch('n')), Action::None));
        assert_eq!(app.mode, Mode::Normal, "uninstall cancelled");
        app.handle_key(ch('U'));
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
