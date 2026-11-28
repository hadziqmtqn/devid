//! TUI state and key handling. Pure: no terminal, no daemon, no side effects. Actions come out
//! as `Effect`s that mod.rs carries out, so everything here is tested directly.

use devid_daemon::{
    newproject::{self, TEMPLATES},
    protocol::Request,
    sites,
};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Sites,
    Php,
    Server,
    Mail,
    Databases,
    Logs,
    Settings,
}

impl Page {
    pub const ALL: [Page; 7] =
        [Page::Sites, Page::Php, Page::Server, Page::Mail, Page::Databases, Page::Logs, Page::Settings];

    pub fn title(self) -> &'static str {
        match self {
            Page::Sites => "Sites",
            Page::Php => "PHP",
            Page::Server => "Server",
            Page::Mail => "Mail",
            Page::Databases => "Databases",
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
    /// Share a site through a Cloudflare quick tunnel and show its link (D-037).
    Share(String),
    /// Load an engine's accounts (D-061), then show the Accounts view.
    LoadAccounts(String),
    /// Make an account (`DbUserAdd`) or give one a new password (`DbUserPassword`), then show the
    /// password and the account's `.env` lines.
    DbAccount(Request),
    /// Copy an account's `.env` lines, or its password, as `db_env` gives them (default: the main
    /// account).
    DbCopy {
        engine: String,
        name: Option<String>,
        password: bool,
    },
    /// Copy text that is on the screen already; the message names `what` rather than the text.
    CopyQuiet {
        text: String,
        what: String,
    },
    /// D-063: make the project the wizard describes, with its steps on the message line; then show
    /// what was made, or the wizard again with the error.
    NewProject(Box<NewView>),
}

/// A field of the New project wizard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewField {
    Template,
    Name,
    Kit,
    Db,
    Create,
}

/// The New project wizard (D-063, `n` on the Sites page): a template from `newproject::TEMPLATES`,
/// a name, and the template's kit and database; indexes into the template's lists.
#[derive(Debug, Clone, PartialEq)]
pub struct NewView {
    pub template: usize,
    pub name: String,
    pub kit: usize,
    pub db: usize,
    pub field: NewField,
    pub error: Option<String>,
}

impl Default for NewView {
    fn default() -> Self {
        let laravel = TEMPLATES.iter().position(|t| t.key == "laravel").unwrap_or(0);
        Self { template: laravel, name: String::new(), kit: 0, db: 0, field: NewField::Template, error: None }
    }
}

impl NewView {
    pub fn template(&self) -> &'static newproject::Template {
        &TEMPLATES[self.template % TEMPLATES.len()]
    }

    /// The kit shown, "" for the template's default.
    pub fn kit(&self) -> &'static str {
        self.template().kits.get(self.kit).copied().unwrap_or("")
    }

    /// The database shown, if the template offers a choice.
    pub fn db(&self) -> Option<&'static str> {
        let t = self.template();
        let skeleton = t.key == "symfony" && self.kit() == "skeleton";
        (t.dbs.len() > 1 && !skeleton).then(|| t.dbs.get(self.db).copied().unwrap_or(t.dbs[0]))
    }

    /// The fields this template has, in order.
    pub fn fields(&self) -> Vec<NewField> {
        let mut fields = vec![NewField::Template, NewField::Name];
        if !self.template().kits.is_empty() {
            fields.push(NewField::Kit);
        }
        if self.db().is_some() {
            fields.push(NewField::Db);
        }
        fields.push(NewField::Create);
        fields
    }

    fn cycle(&mut self, delta: isize) {
        let step = |i: usize, n: usize| (i as isize + delta).rem_euclid(n.max(1) as isize) as usize;
        match self.field {
            NewField::Template => {
                self.template = step(self.template, TEMPLATES.len());
                (self.kit, self.db) = (0, 0);
            }
            NewField::Kit => self.kit = step(self.kit, self.template().kits.len()),
            NewField::Db => self.db = step(self.db, self.template().dbs.len()),
            NewField::Name | NewField::Create => {}
        }
    }

    /// The request this wizard sends; the template's defaults are left out, as the CLI does.
    pub fn request(&self) -> Request {
        let t = self.template();
        Request::NewProject {
            template: t.key.into(),
            name: self.name.clone(),
            path: None,
            php: None,
            db: self.db().filter(|d| Some(*d) != t.dbs.first().copied()).map(str::to_string),
            kit: Some(self.kit()).filter(|k| !k.is_empty()).map(str::to_string),
        }
    }
}

/// One account in the Accounts view (D-061).
#[derive(Debug, Clone, PartialEq)]
pub struct AccountRow {
    pub name: String,
    pub database: Option<String>,
    /// "kept" (Devid set it), "none" (the main account, trusted from this computer), "unknown".
    pub password: String,
    pub main: bool,
}

/// One engine's accounts: add one, give one a new password, copy its `.env` lines, remove it.
#[derive(Debug, Clone, PartialEq)]
pub struct AccountsView {
    pub engine: String,
    pub label: String,
    pub series: String,
    pub rows: Vec<AccountRow>,
    pub index: usize,
}

/// A password Devid just set, with the account's `.env` lines.
#[derive(Debug, Clone, PartialEq)]
pub struct Secret {
    pub engine: String,
    pub title: String,
    pub name: String,
    pub password: String,
    pub env: Vec<String>,
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputFor {
    Link,
    Park,
    /// A site's local port (D-037): a port, "off", or empty for the first free one from 8000.
    Port(String),
    /// A new command for a site (D-036): its name, a space, then the shell line.
    Command(String),
    /// A new account on this engine (D-061): its name, and optionally a space and its database.
    Account(String),
    /// A new database on this engine.
    Database(String),
    /// This engine's port.
    DbPort(String),
}

/// What a pick list is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickFor {
    /// Isolate this site to the picked PHP version.
    Isolate(String),
    Unpark,
    /// Serve an ignored parked site again (D-029).
    Unignore,
    /// A site's app type (D-035): option 0 follows detection, then `sites::PRESETS` in order.
    Preset(String),
    /// A site's serving mode, `sites::Mode::NAMES` in order.
    Serve(String),
    /// Start or stop one of a site's commands (D-036); `running` per name.
    Command {
        site: String,
        names: Vec<String>,
        running: Vec<bool>,
    },
    /// Add one of the site's suggested commands (name, line); the last option types one in.
    Suggest {
        site: String,
        items: Vec<(String, String)>,
    },
    /// Remove one of a site's commands, after a confirmation.
    RemoveCommand {
        site: String,
        names: Vec<String>,
    },
    /// D-054: the command that starts with the site's first visit, or Off (the last option);
    /// `idle` is the current `idle_stop`, the next pick's default.
    OnDemand {
        site: String,
        names: Vec<String>,
        idle: Option<u64>,
    },
    /// D-054: how long that command may be idle before it stops, `ON_DEMAND_IDLE` in order.
    OnDemandIdle {
        site: String,
        command: String,
    },
}

/// The idle times the TUI offers for an on-demand command (minutes, label); 0 never stops it.
const ON_DEMAND_IDLE: [(u32, &str); 6] = [
    (5, "After 5 min idle"),
    (10, "After 10 min idle"),
    (15, "After 15 min idle"),
    (30, "After 30 min idle"),
    (60, "After 1 hour idle"),
    (0, "Never"),
];

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
    Accounts(AccountsView),
    /// A new password or a removal from the Accounts view waiting for y/n; no goes back to the view.
    ConfirmAccount {
        question: String,
        request: Box<Request>,
        done: String,
        back: Box<AccountsView>,
    },
    /// A password Devid just set; closing goes back to the Accounts view.
    Secret(Secret),
    /// The New project wizard.
    NewProject(NewView),
    /// What `devid new` made: the site's URL and the lines the CLI prints.
    NewDone {
        title: String,
        url: String,
        lines: Vec<String>,
    },
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
    /// Names of ignored parked sites (D-029).
    pub ignored: Vec<String>,
    /// `server_list` entries: nginx and caddy, and apache on macOS.
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
    /// After this action, load this engine's Accounts view again.
    pub reopen_accounts: Option<String>,
}

/// `AppSet` changing only the preset or the mode of `site`.
fn app_set(site: &str, preset: Option<String>, mode: Option<String>) -> Request {
    Request::AppSet {
        site: Some(site.to_string()),
        cwd: String::new(),
        preset,
        mode,
        root: None,
        upstream: None,
        spa: None,
        pass_host: None,
        on_demand: None,
        idle_stop: None,
    }
}

/// `AppSet` changing only on-demand (D-054): a command name, or "" for off.
fn on_demand_set(site: &str, command: &str, idle_stop: Option<u32>) -> Request {
    Request::AppSet {
        site: Some(site.to_string()),
        cwd: String::new(),
        preset: None,
        mode: None,
        root: None,
        upstream: None,
        spa: None,
        pass_host: None,
        on_demand: Some(command.to_string()),
        idle_stop,
    }
}

/// "stops after 10 min idle", or "keeps running" for 0.
fn idle_text(minutes: Option<u64>) -> String {
    match minutes {
        Some(0) => "keeps running".to_string(),
        Some(m) => format!("stops after {m} min idle"),
        None => String::new(),
    }
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
            ignored: Vec::new(),
            servers: Vec::new(),
            logs: Vec::new(),
            log_text: String::new(),
            follow: false,
            checks: Vec::new(),
            reopen: None,
            reopen_accounts: None,
        }
    }

    /// Show the Accounts view from a `db_user_list` reply.
    pub fn open_accounts(&mut self, list: &Value) {
        let engine = s(&list["engine"]);
        let rows = list["users"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|u| AccountRow {
                name: s(&u["name"]),
                database: u["database"].as_str().map(String::from),
                password: s(&u["password"]),
                main: u["main"] == true,
            })
            .collect();
        self.mode = Mode::Accounts(AccountsView {
            label: engine_label(&engine),
            engine,
            series: s(&list["series"]),
            rows,
            index: 0,
        });
    }

    /// Show the password from a `db_user_add` or `db_user_password` reply.
    pub fn open_secret(&mut self, v: &Value, added: bool) {
        let (engine, name) = (s(&v["engine"]), s(&v["name"]));
        let title = match (added, v["database"].as_str()) {
            (true, Some(db)) => format!("New {} account {name}, with the database {db}", engine_label(&engine)),
            (true, None) => format!("New {} account {name}", engine_label(&engine)),
            (false, _) => format!("New password for {name} ({})", engine_label(&engine)),
        };
        let env = v["env"].as_array().into_iter().flatten().map(s).collect();
        self.mode = Mode::Secret(Secret { engine, title, name, password: s(&v["password"]), env });
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
            Page::Databases => self.databases().len(),
            Page::Mail | Page::Settings => 0,
        }
    }

    /// `status.databases` (D-061): one entry per engine, or none without a daemon (or with one older
    /// than databases).
    pub fn databases(&self) -> &[Value] {
        self.status.as_ref().ok().and_then(|st| st["databases"].as_array()).map_or(&[], Vec::as_slice)
    }

    /// Actions for a database row.
    pub fn db_menu(d: &Value) -> Vec<(char, String)> {
        let label = s(&d["label"]);
        if d["enabled"] != true {
            let first = match d["installed"].as_array().is_some_and(|l| !l.is_empty()) {
                true => "",
                false => " (the first time, Devid downloads it)",
            };
            return vec![('s', format!("Start {label}{first}"))];
        }
        let main = devid_daemon::db::Engine::parse(&s(&d["engine"])).map_or("root", devid_daemon::db::main_account);
        vec![
            ('a', "Accounts… (add, new password, copy .env)".to_string()),
            ('c', format!("Copy the .env lines of {main}, the main account")),
            ('n', "New database…".to_string()),
            ('o', format!("Port… ({})", d["port"])),
            ('r', format!("Restart {label}")),
            match d["state"] == "paused" {
                true => ('s', format!("Start {label}")),
                false => ('s', format!("Stop {label} (its data stays)")),
            },
        ]
    }

    fn db_action(&mut self, row: usize, choice: char) -> Action {
        let Some(d) = self.databases().get(row).cloned() else { return Action::None };
        let (engine, label) = (s(&d["engine"]), s(&d["label"]));
        let daemon = |request, done: String| Action::Do(Effect::Daemon { request, done });
        match choice {
            's' if d["enabled"] != true => daemon(
                Request::DbStart { engine: engine.clone(), series: None },
                format!("{label} runs; enter → Accounts… adds one"),
            ),
            's' if d["state"] == "paused" => {
                daemon(Request::ServiceStart { service: engine.clone() }, format!("started {label}"))
            }
            's' => {
                self.mode = Mode::Confirm {
                    question: format!("Stop {label}? Apps cannot reach it until you start it again; its data stays."),
                    request: Box::new(Request::DbStop { engine }),
                    done: format!("{label} is off; its data stays"),
                };
                Action::None
            }
            'r' => daemon(Request::Restart { service: engine }, format!("restarted {label}")),
            'a' => Action::Do(Effect::LoadAccounts(engine)),
            'c' => Action::Do(Effect::DbCopy { engine, name: None, password: false }),
            'n' => {
                self.mode = Mode::Input { purpose: InputFor::Database(engine), text: String::new() };
                Action::None
            }
            'o' => {
                let text = d["port"].as_u64().map(|p| p.to_string()).unwrap_or_default();
                self.mode = Mode::Input { purpose: InputFor::DbPort(engine), text };
                Action::None
            }
            _ => Action::None,
        }
    }

    /// Keys in the Accounts view (D-061).
    fn accounts_key(&mut self, key: KeyEvent, mut v: AccountsView) -> Action {
        self.message = None;
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return Action::None,
            KeyCode::Up | KeyCode::Char('k') => v.index = v.index.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => v.index = (v.index + 1).min(v.rows.len().saturating_sub(1)),
            KeyCode::Char('a') => {
                self.mode = Mode::Input { purpose: InputFor::Account(v.engine), text: String::new() };
                return Action::None;
            }
            KeyCode::Char(c @ ('p' | 'c' | 'y' | 'd')) => {
                if let Some(row) = v.rows.get(v.index).cloned() {
                    let (engine, name) = (v.engine.clone(), row.name.clone());
                    match c {
                        'c' | 'y' => {
                            self.mode = Mode::Accounts(v);
                            return Action::Do(Effect::DbCopy { engine, name: Some(name), password: c == 'y' });
                        }
                        'p' => {
                            self.mode = Mode::ConfirmAccount {
                                question: format!(
                                    "Give {name} a new password? Apps that use the old one cannot connect until their .env has the new one."
                                ),
                                request: Box::new(Request::DbUserPassword { engine, name, password: None }),
                                done: String::new(),
                                back: Box::new(v),
                            };
                            return Action::None;
                        }
                        _ if row.main => self.message = Some((format!("{name} is the main account; it stays"), true)),
                        _ => {
                            self.mode = Mode::ConfirmAccount {
                                question: format!("Remove the account {name}? Its databases and their data stay."),
                                request: Box::new(Request::DbUserRemove { engine, name: name.clone() }),
                                done: format!("removed {name}; its databases stay"),
                                back: Box::new(v),
                            };
                            return Action::None;
                        }
                    }
                }
            }
            _ => {}
        }
        self.mode = Mode::Accounts(v);
        Action::None
    }

    /// `status.mail` (D-049), or Null without a daemon (or with one older than Mail).
    pub fn mail(&self) -> &Value {
        static NONE: Value = Value::Null;
        self.status.as_ref().map_or(&NONE, |s| &s["mail"])
    }

    /// Mail page keys; None for a key the page does not use.
    fn mail_key(&mut self, c: char) -> Option<Action> {
        let m = self.mail().clone();
        let running = m["state"] == "running";
        let set = |enabled, php_mail| Request::MailSet { enabled, port: None, keep: None, php_mail, notify: None };
        let daemon = |request, done: &str| Some(Action::Do(Effect::Daemon { request, done: done.into() }));
        let not_running = |app: &mut Self| {
            let off = m["enabled"] != true;
            app.message = Some((if off { "Mail is off; m turns it on" } else { "Mail is not running" }.into(), true));
            Some(Action::None)
        };
        match c {
            'm' if m["enabled"] == true => daemon(set(Some(false), None), "Mail is off; the mail it caught stays"),
            'm' => daemon(set(Some(true), None), "Mail is on"),
            't' if running => daemon(Request::MailTest, "sent a test email to test@example.com"),
            'o' if running => {
                Some(Action::Do(Effect::OpenUrl(m["inbox_url"].as_str().unwrap_or_default().to_string())))
            }
            't' | 'o' | 'X' => not_running(self),
            'c' => {
                let env = format!(
                    "MAIL_MAILER=smtp\nMAIL_HOST=127.0.0.1\nMAIL_PORT={}\n",
                    m["port"].as_u64().unwrap_or(2525)
                );
                Some(Action::Do(Effect::Copy(env)))
            }
            'p' => {
                let on = m["php_mail"] != false;
                let done = if on { "PHP's mail() no longer goes to Mail" } else { "PHP's mail() goes to Mail" };
                daemon(set(None, Some(!on)), done)
            }
            _ => None,
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
        // Phase 8: the app type, the local port and the site's commands.
        items.push(('y', format!("App type… ({})", site["preset_label"].as_str().unwrap_or("PHP"))));
        items.push(('m', format!("Serve as… ({})", site["mode"].as_str().unwrap_or("php"))));
        items.push(('p', "Local port…".to_string()));
        let cmds = site["commands"].as_array().map(Vec::as_slice).unwrap_or_default();
        let running = cmds.iter().filter(|c| c["state"] == "running").count();
        if !cmds.is_empty() {
            items.push(('g', format!("Commands… ({running} of {} running)", cmds.len())));
            if running < cmds.len() {
                items.push(('a', "Start all commands".to_string()));
            }
            if running > 0 {
                items.push(('s', "Stop all commands".to_string()));
            }
            items.push(('x', "Remove a command…".to_string()));
        }
        // D-054: one command of a proxy site can start with the site's first visit.
        if site["mode"] == "proxy" && !cmds.is_empty() {
            let now = match site["on_demand"]["command"].as_str() {
                Some(c) => format!("{c}, {}", idle_text(site["on_demand"]["idle_stop"].as_u64())),
                None => "off".to_string(),
            };
            items.push(('z', format!("On demand… ({now})")));
        }
        items.push(('n', "Add a command…".to_string()));
        match site["share"]["url"].as_str() {
            Some(url) => items.push(('h', format!("Stop sharing {url}"))),
            None if site["share"]["state"] == "starting" => items.push(('h', "Stop sharing (connecting)".to_string())),
            None => items.push(('h', "Share on the internet (trycloudflare.com)".to_string())),
        }
        if site["linked"] == true {
            items.push(('d', "Unlink…".to_string()));
        } else {
            items.push(('r', "Remove site… (the folder stays)".to_string()));
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
            Mode::NewProject(view) => self.new_key(key, view),
            Mode::NewDone { title, url, lines } => match key.code {
                KeyCode::Char('o') => {
                    self.mode = Mode::NewDone { title, url: url.clone(), lines };
                    Action::Do(Effect::OpenUrl(url))
                }
                KeyCode::Char('c') => {
                    let text = format!("{url}\n{}\n", lines.join("\n"));
                    self.mode = Mode::NewDone { title: title.clone(), url, lines };
                    Action::Do(Effect::CopyQuiet { text, what: format!("{title}'s details") })
                }
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => Action::Refresh,
                _ => {
                    self.mode = Mode::NewDone { title, url, lines };
                    Action::None
                }
            },
            Mode::Accounts(view) => self.accounts_key(key, view),
            Mode::ConfirmAccount { question, request, done, back } => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => match *request {
                    // A new password shows itself, then goes back to the view.
                    r @ Request::DbUserPassword { .. } => Action::Do(Effect::DbAccount(r)),
                    r => {
                        self.reopen_accounts = Some(back.engine.clone());
                        Action::Do(Effect::Daemon { request: r, done })
                    }
                },
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.mode = Mode::Accounts(*back);
                    Action::None
                }
                _ => {
                    self.mode = Mode::ConfirmAccount { question, request, done, back };
                    Action::None
                }
            },
            Mode::Secret(secret) => match key.code {
                KeyCode::Char(c @ ('c' | 'y')) => {
                    let (text, what) = match c {
                        'c' => (format!("{}\n", secret.env.join("\n")), format!("{}'s .env lines", secret.name)),
                        _ => (secret.password.clone(), format!("{}'s password", secret.name)),
                    };
                    self.mode = Mode::Secret(secret);
                    Action::Do(Effect::CopyQuiet { text, what })
                }
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => Action::Do(Effect::LoadAccounts(secret.engine)),
                _ => {
                    self.mode = Mode::Secret(secret);
                    Action::None
                }
            },
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
            KeyCode::Char(c @ '1'..='7') => self.select_page(Page::ALL[c as usize - '1' as usize]),
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
            KeyCode::Enter if self.page == Page::Databases && self.focus == Focus::Page => {
                if let Some(d) = self.databases().get(self.row) {
                    let items = Self::db_menu(d);
                    self.mode = Mode::Menu { row: self.row, items, index: 0 };
                }
            }
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
            KeyCode::Char('X') if self.page == Page::Mail && self.mail()["state"] == "running" => {
                self.mode = Mode::Confirm {
                    question: "Delete all mail? It is deleted from ~/.devid/mail.".into(),
                    request: Box::new(Request::MailClear),
                    done: "deleted all mail".into(),
                };
            }
            KeyCode::Char(c) if self.page == Page::Mail => {
                if let Some(action) = self.mail_key(c) {
                    return action;
                }
            }
            KeyCode::Char('d') if self.page == Page::Settings => return Action::Do(Effect::Doctor),
            KeyCode::Char('a') if self.page == Page::Settings => {
                let on = self.status.as_ref().is_ok_and(|s| s["autostart"] != false);
                let done =
                    if on { "after login, nginx and PHP wait for start all" } else { "sites are served at login" };
                return Action::Do(Effect::Daemon { request: Request::Autostart { enabled: !on }, done: done.into() });
            }
            KeyCode::Char('p') if self.page == Page::Settings => {
                // D-053: Devid's commands on PATH for new terminals, or not.
                let on = self.status.as_ref().is_ok_and(|s| s["terminal"]["on"] == true);
                let done = if on {
                    "new terminals no longer get Devid's commands"
                } else {
                    "Devid's commands are on PATH; open a new terminal to use them"
                };
                return Action::Do(Effect::Daemon { request: Request::ShellPath { enabled: !on }, done: done.into() });
            }
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
            KeyCode::Char('n') if self.page == Page::Sites => self.mode = Mode::NewProject(NewView::default()),
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
            KeyCode::Char('I') if self.page == Page::Sites => {
                if self.ignored.is_empty() {
                    self.message = Some(("No ignored sites.".into(), true));
                } else {
                    self.mode = Mode::Pick { purpose: PickFor::Unignore, options: self.ignored.clone(), index: 0 };
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
        if self.page == Page::Databases {
            return self.db_action(row, choice);
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
            'y' => {
                let detected = site["detected"].as_str().and_then(sites::find_preset).map_or("nothing", |p| p.label);
                let current = s(&site["preset"]);
                let mut options = vec![format!("Use detected ({detected})")];
                options.extend(
                    sites::PRESETS.iter().map(|p| format!("{}{}", p.label, if p.key == current { "  ✓" } else { "" })),
                );
                self.mode = Mode::Pick { purpose: PickFor::Preset(name), options, index: 0 };
                Action::None
            }
            'm' => {
                let current = s(&site["mode"]);
                let options = sites::Mode::NAMES
                    .iter()
                    .map(|m| format!("{m}{}", if *m == current { "  ✓" } else { "" }))
                    .collect();
                self.mode = Mode::Pick { purpose: PickFor::Serve(name), options, index: 0 };
                Action::None
            }
            'p' => {
                let text = site["port"].as_u64().map(|p| p.to_string()).unwrap_or_default();
                self.mode = Mode::Input { purpose: InputFor::Port(name), text };
                Action::None
            }
            'g' | 'x' => {
                let cmds = site["commands"].as_array().cloned().unwrap_or_default();
                let names: Vec<String> = cmds.iter().map(|c| s(&c["name"])).collect();
                let running: Vec<bool> = cmds.iter().map(|c| c["state"] == "running").collect();
                let on_demand = site["on_demand"]["command"].as_str();
                let options = cmds
                    .iter()
                    .map(|c| match choice {
                        'g' => {
                            let state = s(&c["state"]);
                            let state = match (on_demand == c["name"].as_str(), state.as_str()) {
                                (true, "stopped") => "sleeping, on demand".to_string(),
                                (true, _) => format!("{state}, on demand"),
                                (false, _) => state,
                            };
                            format!("{}  ·  {state}", s(&c["name"]))
                        }
                        _ => s(&c["name"]),
                    })
                    .collect();
                let purpose = match choice {
                    'g' => PickFor::Command { site: name, names, running },
                    _ => PickFor::RemoveCommand { site: name, names },
                };
                self.mode = Mode::Pick { purpose, options, index: 0 };
                Action::None
            }
            'z' => {
                let names: Vec<String> =
                    site["commands"].as_array().into_iter().flatten().map(|c| s(&c["name"])).collect();
                let current = site["on_demand"]["command"].as_str();
                let mark = |on: bool| if on { "  ✓" } else { "" };
                let mut options: Vec<String> =
                    names.iter().map(|n| format!("{n}{}", mark(Some(n.as_str()) == current))).collect();
                options.push(format!("Off{}", mark(current.is_none())));
                let index = current.and_then(|c| names.iter().position(|n| n == c)).unwrap_or(names.len());
                let idle = site["on_demand"]["idle_stop"].as_u64();
                self.mode = Mode::Pick { purpose: PickFor::OnDemand { site: name, names, idle }, options, index };
                Action::None
            }
            'h' if site["share"].is_object() && site["share"]["state"] != "exited" => Action::Do(Effect::Daemon {
                request: Request::ShareStop { site: Some(name.clone()), cwd: String::new() },
                done: format!("stopped sharing {name}"),
            }),
            'h' => Action::Do(Effect::Share(name)),
            'a' | 's' => Action::Do(Effect::Daemon {
                request: match choice {
                    'a' => Request::CommandStart { site: Some(name.clone()), cwd: String::new(), name: None },
                    _ => Request::CommandStop { site: Some(name.clone()), cwd: String::new(), name: None },
                },
                done: format!("{} {name}'s commands", if choice == 'a' { "started" } else { "stopped" }),
            }),
            'n' => {
                let items: Vec<(String, String)> = site["suggestions"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|c| (s(&c["name"]), s(&c["run"])))
                    .collect();
                self.mode = if items.is_empty() {
                    Mode::Input { purpose: InputFor::Command(name), text: String::new() }
                } else {
                    let mut options: Vec<String> = items.iter().map(|(n, r)| format!("{n}  —  {r}")).collect();
                    options.push("Other… (type a name and a command)".to_string());
                    Mode::Pick { purpose: PickFor::Suggest { site: name, items }, options, index: 0 }
                };
                Action::None
            }
            'r' => {
                self.mode = Mode::Confirm {
                    question: format!("Remove {name}? Devid stops serving it; the folder itself is not touched."),
                    request: Box::new(Request::Ignore { site: name.clone() }),
                    done: format!("ignored {name} (I serves it again)"),
                };
                Action::None
            }
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
    /// Keys in the New project wizard: ↑↓ (tab) move between fields, ←→ change a choice, letters
    /// type the name, enter on Create makes the project.
    fn new_key(&mut self, key: KeyEvent, mut v: NewView) -> Action {
        let fields = v.fields();
        let pos = fields.iter().position(|f| *f == v.field).unwrap_or(0);
        let next = fields[(pos + 1).min(fields.len() - 1)];
        match key.code {
            KeyCode::Esc => return Action::None,
            KeyCode::Up | KeyCode::BackTab => v.field = fields[pos.saturating_sub(1)],
            KeyCode::Down | KeyCode::Tab => v.field = next,
            KeyCode::Left if v.field != NewField::Name => v.cycle(-1),
            KeyCode::Right if v.field != NewField::Name => v.cycle(1),
            KeyCode::Enter if v.field == NewField::Create => match newproject::check_name(&v.name) {
                Ok(()) => return Action::Do(Effect::NewProject(Box::new(v))),
                Err(e) => {
                    v.error = Some(e);
                    v.field = NewField::Name;
                }
            },
            KeyCode::Enter => v.field = next,
            KeyCode::Backspace if v.field == NewField::Name => {
                v.name.pop();
            }
            KeyCode::Char(c) if v.field == NewField::Name => {
                let c = c.to_ascii_lowercase();
                if (c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') && v.name.len() < 32 {
                    v.name.push(c);
                    v.error = None;
                }
            }
            KeyCode::Char('h') if v.field != NewField::Name => v.cycle(-1),
            KeyCode::Char('l') if v.field != NewField::Name => v.cycle(1),
            KeyCode::Char('k') if v.field != NewField::Name => v.field = fields[pos.saturating_sub(1)],
            KeyCode::Char('j') if v.field != NewField::Name => v.field = next,
            _ => {}
        }
        // A kit that hides the database (Symfony's skeleton) moves focus off it.
        if !v.fields().contains(&v.field) {
            v.field = NewField::Create;
        }
        self.mode = Mode::NewProject(v);
        Action::None
    }

    /// Show what `NewProject` made, as the CLI prints it.
    pub fn open_new_done(&mut self, v: &Value) {
        let mut lines = vec![format!("folder   {}", s(&v["path"]))];
        if v["php"].is_string() {
            lines.push(format!("PHP      {}", s(&v["php"])));
        }
        if let Some(port) = v["port"].as_u64() {
            lines.push(format!("dev      port {port}, started by the site's first visit"));
        }
        if v["account"].is_object() {
            let a = &v["account"];
            lines.push(format!(
                "database {} account {}, password {}",
                s(&a["label"]),
                s(&a["name"]),
                s(&a["password"])
            ));
        }
        if v["admin"].is_object() {
            lines.push(format!(
                "sign in  {} / {}  (shown once: Devid does not keep it)",
                s(&v["admin"]["user"]),
                s(&v["admin"]["password"])
            ));
        }
        for c in v["commands"].as_array().into_iter().flatten() {
            lines.push(format!("command  {}: {}", s(&c["name"]), s(&c["run"])));
        }
        for note in v["notes"].as_array().into_iter().flatten() {
            lines.push(format!("note     {}", s(note)));
        }
        lines.push(format!("log      {}", s(&v["log"])));
        self.mode = Mode::NewDone { title: format!("{} is ready", s(&v["name"])), url: s(&v["url"]), lines };
    }

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
            // A new account goes back to the Accounts view it came from.
            KeyCode::Esc => {
                return match purpose {
                    InputFor::Account(engine) => Action::Do(Effect::LoadAccounts(engine)),
                    _ => Action::None,
                };
            }
            KeyCode::Enter => {
                let text = text.trim();
                let (request, done) = match &purpose {
                    InputFor::Link | InputFor::Park => {
                        let path = expand_tilde(text);
                        if path.is_empty() {
                            return Action::None;
                        }
                        match purpose {
                            InputFor::Link => {
                                (Request::Link { name: None, path: path.clone() }, format!("linked {path}"))
                            }
                            _ => (Request::Park { path: path.clone() }, format!("parked {path}")),
                        }
                    }
                    InputFor::Port(site) => {
                        let port = match text {
                            "" => None,
                            "off" => Some(0),
                            t => match t.parse::<u16>() {
                                Ok(p) => Some(p),
                                Err(_) => {
                                    self.message = Some((format!("{t:?} is not a port (1024–65535) or off"), true));
                                    return Action::None;
                                }
                            },
                        };
                        let done = match port {
                            Some(0) => format!("{site}: local port off"),
                            Some(p) => format!("{site} is also served on 127.0.0.1:{p}"),
                            None => format!("{site} is also served on a local port (see the list)"),
                        };
                        (Request::AppPort { site: Some(site.clone()), cwd: String::new(), port }, done)
                    }
                    InputFor::Command(site) => {
                        let Some((name, run)) = text.split_once(char::is_whitespace) else {
                            self.message =
                                Some(("type the name, a space, then the command (dev npm run dev)".into(), true));
                            return Action::None;
                        };
                        let request = Request::CommandAdd {
                            site: Some(site.clone()),
                            cwd: String::new(),
                            name: name.to_string(),
                            run: run.trim().to_string(),
                        };
                        (request, format!("added {name} to {site}; start it from the site's menu"))
                    }
                    InputFor::Account(engine) => {
                        let mut words = text.split_whitespace();
                        let Some(name) = words.next() else { return Action::Do(Effect::LoadAccounts(engine.clone())) };
                        return Action::Do(Effect::DbAccount(Request::DbUserAdd {
                            engine: engine.clone(),
                            name: name.to_string(),
                            database: words.next().map(String::from),
                            password: None,
                            no_database: false,
                            create_databases: false,
                        }));
                    }
                    InputFor::Database(engine) => {
                        if text.is_empty() {
                            return Action::None;
                        }
                        let done = match engine.as_str() {
                            "mongodb" => format!("MongoDB makes {text} when an app first writes to it"),
                            _ => format!("made the {} database {text}", engine_label(engine)),
                        };
                        (Request::DbCreate { engine: engine.clone(), database: text.to_string() }, done)
                    }
                    InputFor::DbPort(engine) => {
                        let Ok(port) = text.parse::<u16>() else {
                            self.message = Some((format!("{text:?} is not a port (1024–65535)"), true));
                            return Action::None;
                        };
                        let done = format!("{} is on port {port}; update DB_PORT in your apps", engine_label(engine));
                        (Request::DbPort { engine: engine.clone(), port }, done)
                    }
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
                    PickFor::Unignore => Effect::Daemon {
                        request: Request::Unignore { site: choice.clone() },
                        done: format!("serving {choice} again"),
                    },
                    PickFor::Preset(site) => match index.checked_sub(1).and_then(|i| sites::PRESETS.get(i)) {
                        None => Effect::Daemon {
                            request: Request::AppReset { site: Some(site.clone()), cwd: String::new() },
                            done: format!("{site} follows detection again"),
                        },
                        Some(p) => Effect::Daemon {
                            request: app_set(&site, Some(p.key.to_string()), None),
                            done: format!("{site} is now {}", p.label),
                        },
                    },
                    PickFor::Serve(site) => {
                        let mode = sites::Mode::NAMES[index.min(2)];
                        Effect::Daemon {
                            request: app_set(&site, None, Some(mode.to_string())),
                            done: format!("{site} is served as {mode}"),
                        }
                    }
                    PickFor::Command { site, names, running } => {
                        let name = names[index].clone();
                        let (request, verb) = match running[index] {
                            true => (
                                Request::CommandStop { site: Some(site), cwd: String::new(), name: Some(name.clone()) },
                                "stopped",
                            ),
                            false => (
                                Request::CommandStart {
                                    site: Some(site),
                                    cwd: String::new(),
                                    name: Some(name.clone()),
                                },
                                "started",
                            ),
                        };
                        Effect::Daemon { request, done: format!("{verb} {name}") }
                    }
                    PickFor::Suggest { site, items } => match items.get(index) {
                        Some((name, run)) => Effect::Daemon {
                            request: Request::CommandAdd {
                                site: Some(site.clone()),
                                cwd: String::new(),
                                name: name.clone(),
                                run: run.clone(),
                            },
                            done: format!("added {name} to {site}; start it from the site's menu"),
                        },
                        None => {
                            self.mode = Mode::Input { purpose: InputFor::Command(site), text: String::new() };
                            return Action::None;
                        }
                    },
                    PickFor::RemoveCommand { site, names } => {
                        let name = names[index].clone();
                        self.mode = Mode::Confirm {
                            question: format!("Remove {name}? A running one is stopped first; its log stays."),
                            request: Box::new(Request::CommandRemove {
                                site: Some(site),
                                cwd: String::new(),
                                name: name.clone(),
                            }),
                            done: format!("removed {name}"),
                        };
                        return Action::None;
                    }
                    PickFor::OnDemand { site, names, idle } => match names.get(index) {
                        Some(command) => {
                            let options = ON_DEMAND_IDLE.iter().map(|(_, label)| label.to_string()).collect();
                            let index = idle
                                .and_then(|m| ON_DEMAND_IDLE.iter().position(|(v, _)| u64::from(*v) == m))
                                .unwrap_or(1);
                            let purpose = PickFor::OnDemandIdle { site, command: command.clone() };
                            self.mode = Mode::Pick { purpose, options, index };
                            return Action::None;
                        }
                        None => Effect::Daemon {
                            request: on_demand_set(&site, "", None),
                            done: format!("{site} is not on demand; its commands start when you say"),
                        },
                    },
                    PickFor::OnDemandIdle { site, command } => {
                        let minutes = ON_DEMAND_IDLE[index.min(ON_DEMAND_IDLE.len() - 1)].0;
                        let done = match minutes {
                            0 => format!("{command} starts with the first visit to {site} and keeps running"),
                            m => {
                                format!("{command} starts with the first visit to {site} and stops after {m} min idle")
                            }
                        };
                        Effect::Daemon { request: on_demand_set(&site, &command, Some(minutes)), done }
                    }
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

/// "MySQL" for "mysql".
fn engine_label(engine: &str) -> String {
    devid_daemon::db::Engine::parse(engine).map_or_else(|| engine.to_string(), |e| e.label().to_string())
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
        assert_eq!(keys(&app.sites[0]), "otciympnhr", "parked, default PHP: no unisolate, no unlink, remove (D-029)");
        assert_eq!(keys(&app.sites[1]), "otciuympnhd", "linked + isolated");
    }

    /// Phase 8 A5: the app type, serving mode, local port and commands from a site's menu.
    #[test]
    fn app_type_port_and_commands() {
        let mut app = with_sites();
        let blog = &mut app.sites[0];
        blog["preset"] = json!("laravel");
        blog["preset_label"] = json!("Laravel");
        blog["detected"] = json!("laravel");
        blog["mode"] = json!("php");
        blog["commands"] = json!([{"name": "dev", "state": "running"}, {"name": "queue", "state": "stopped"}]);
        blog["suggestions"] = json!([{"name": "schedule", "run": "php artisan schedule:work"}]);
        let keys: String = App::site_menu(&app.sites[0]).iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, "otciympgasxnhr");
        let menu = |app: &mut App, letter: char| {
            app.handle_key(key(KeyCode::Enter));
            app.handle_key(ch(letter))
        };
        let typed = |app: &mut App, text: &str| {
            for c in text.chars() {
                app.handle_key(ch(c));
            }
            app.handle_key(key(KeyCode::Enter))
        };

        menu(&mut app, 'y');
        assert!(matches!(&app.mode, Mode::Pick { purpose: PickFor::Preset(s), options, .. }
            if s == "blog" && options[0] == "Use detected (Laravel)" && options[1] == "Laravel  ✓"));
        assert_eq!(daemon_json(app.handle_key(key(KeyCode::Enter)))["cmd"], "app_reset", "option 0 follows detection");
        menu(&mut app, 'y');
        for _ in 0..10 {
            app.handle_key(key(KeyCode::Down));
        }
        let v = daemon_json(app.handle_key(key(KeyCode::Enter)));
        assert_eq!(
            (v["cmd"].as_str(), v["preset"].as_str(), v["mode"].as_str()),
            (Some("app_set"), Some("vite"), None)
        );

        menu(&mut app, 'm');
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Down));
        assert_eq!(daemon_json(app.handle_key(key(KeyCode::Enter)))["mode"], "proxy");

        menu(&mut app, 'p');
        assert_eq!(daemon_json(app.handle_key(key(KeyCode::Enter)))["port"], Value::Null, "empty: the first free port");
        menu(&mut app, 'p');
        assert_eq!(daemon_json(typed(&mut app, "off"))["port"], 0);
        menu(&mut app, 'p');
        assert!(matches!(typed(&mut app, "80x"), Action::None));
        assert!(app.message.as_ref().is_some_and(|(m, bad)| *bad && m.contains("not a port")));

        menu(&mut app, 'g');
        let v = daemon_json(app.handle_key(key(KeyCode::Enter)));
        assert_eq!((v["cmd"].as_str(), v["name"].as_str()), (Some("command_stop"), Some("dev")), "a running one stops");
        menu(&mut app, 'g');
        app.handle_key(key(KeyCode::Down));
        let v = daemon_json(app.handle_key(key(KeyCode::Enter)));
        assert_eq!((v["cmd"].as_str(), v["name"].as_str()), (Some("command_start"), Some("queue")));
        let v = daemon_json(menu(&mut app, 'a'));
        assert_eq!(
            (v["cmd"].as_str(), v["site"].as_str(), v["name"].as_str()),
            (Some("command_start"), Some("blog"), None)
        );
        assert_eq!(daemon_json(menu(&mut app, 's'))["cmd"], "command_stop");

        menu(&mut app, 'x');
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Enter));
        assert!(matches!(&app.mode, Mode::Confirm { .. }), "removing asks first");
        let v = daemon_json(app.handle_key(ch('y')));
        assert_eq!((v["cmd"].as_str(), v["name"].as_str()), (Some("command_remove"), Some("queue")));

        menu(&mut app, 'n');
        let v = daemon_json(app.handle_key(key(KeyCode::Enter)));
        assert_eq!((v["name"].as_str(), v["run"].as_str()), (Some("schedule"), Some("php artisan schedule:work")));
        menu(&mut app, 'n');
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Enter));
        assert!(
            matches!(&app.mode, Mode::Input { purpose: InputFor::Command(s), .. } if s == "blog"),
            "Other… types one in"
        );
        let v = daemon_json(typed(&mut app, "dev:pg:local npm run dev:pg:local"));
        assert_eq!(
            (v["cmd"].as_str(), v["name"].as_str(), v["run"].as_str()),
            (Some("command_add"), Some("dev:pg:local"), Some("npm run dev:pg:local"))
        );
        app.mode = Mode::Input { purpose: InputFor::Command("blog".into()), text: String::new() };
        assert!(matches!(typed(&mut app, "justaname"), Action::None), "a name alone is not saved");

        assert!(matches!(menu(&mut app, 'h'), Action::Do(Effect::Share(s)) if s == "blog"), "share");
        app.sites[0]["share"] = json!({"state": "running", "url": "https://a-b.trycloudflare.com"});
        assert!(
            App::site_menu(&app.sites[0]).iter().any(|(k, l)| *k == 'h' && l.contains("https://a-b.trycloudflare.com"))
        );
        assert_eq!(daemon_json(menu(&mut app, 'h'))["cmd"], "share_stop", "a shared site stops sharing");
    }

    /// D-054: a proxy site's command starts with its first visit; `z` picks it, then the idle time.
    #[test]
    fn on_demand_from_the_site_menu() {
        let mut app = with_sites();
        let blog = &mut app.sites[0];
        blog["mode"] = json!("proxy");
        blog["commands"] = json!([{"name": "dev", "state": "stopped"}, {"name": "queue", "state": "running"}]);
        let entry = |app: &App| App::site_menu(&app.sites[0]).into_iter().find(|(k, _)| *k == 'z').map(|(_, l)| l);
        assert_eq!(entry(&app).as_deref(), Some("On demand… (off)"));
        let menu = |app: &mut App| {
            app.handle_key(key(KeyCode::Enter));
            app.handle_key(ch('z'))
        };

        menu(&mut app);
        assert!(
            matches!(&app.mode, Mode::Pick { purpose: PickFor::OnDemand { .. }, options, index: 2 }
            if options == &["dev", "queue", "Off  ✓"]),
            "Off is current: {:?}",
            app.mode
        );
        app.handle_key(key(KeyCode::Up));
        app.handle_key(key(KeyCode::Up));
        app.handle_key(key(KeyCode::Enter));
        assert!(
            matches!(&app.mode, Mode::Pick { purpose: PickFor::OnDemandIdle { command, .. }, options, index: 1 }
                if command == "dev" && options[1] == "After 10 min idle" && options[5] == "Never"),
            "10 min by default: {:?}",
            app.mode
        );
        app.handle_key(key(KeyCode::Down));
        let v = daemon_json(app.handle_key(key(KeyCode::Enter)));
        assert_eq!(
            (v["cmd"].as_str(), v["site"].as_str(), v["on_demand"].as_str(), v["idle_stop"].as_u64()),
            (Some("app_set"), Some("blog"), Some("dev"), Some(15))
        );
        assert_eq!(v["preset"], Value::Null, "nothing else changes");

        // The daemon now reports it on demand and sleeping.
        app.sites[0]["on_demand"] = json!({"command": "dev", "idle_stop": 15, "state": "sleeping"});
        assert_eq!(entry(&app).as_deref(), Some("On demand… (dev, stops after 15 min idle)"));
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(ch('g'));
        assert!(matches!(&app.mode, Mode::Pick { options, .. }
            if options == &["dev  ·  sleeping, on demand", "queue  ·  running"]));
        app.mode = Mode::Normal;
        menu(&mut app);
        assert!(matches!(&app.mode, Mode::Pick { options, index: 0, .. } if options[0] == "dev  ✓"));
        app.handle_key(key(KeyCode::Enter));
        assert!(matches!(&app.mode, Mode::Pick { index: 2, .. }), "the current idle time is the default");
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Down));
        assert_eq!(daemon_json(app.handle_key(key(KeyCode::Enter)))["idle_stop"], 0, "Never");

        menu(&mut app);
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Down));
        let v = daemon_json(app.handle_key(key(KeyCode::Enter)));
        assert_eq!((v["on_demand"].as_str(), v["idle_stop"].as_u64()), (Some(""), None), "Off");

        app.sites[0]["mode"] = json!("php");
        assert_eq!(entry(&app), None, "only proxy sites");
        app.sites[0]["mode"] = json!("proxy");
        app.sites[0]["commands"] = json!([]);
        assert_eq!(entry(&app), None, "and only with a command");
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
    fn remove_and_unignore_a_parked_site() {
        let mut app = with_sites();
        app.handle_key(key(KeyCode::Enter));
        assert!(matches!(app.handle_key(ch('r')), Action::None), "remove asks first");
        assert_eq!(daemon_json(app.handle_key(ch('y'))), json!({"cmd": "ignore", "site": "blog"}));

        app.handle_key(ch('I'));
        assert_eq!(app.message.as_ref().map(|m| m.1), Some(true), "nothing to unignore is reported");
        app.ignored = vec!["api".into()];
        app.handle_key(ch('I'));
        assert_eq!(daemon_json(app.handle_key(key(KeyCode::Enter))), json!({"cmd": "unignore", "site": "api"}));
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
        assert_eq!(app.rows(), 4, "8.3 to 8.6 (D-009, D-062)");

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
        app.status = Ok(json!({"autostart": true}));
        assert_eq!(daemon_json(app.handle_key(ch('a'))), json!({"cmd": "autostart", "enabled": false}), "a toggles");
        app.status = Ok(json!({"autostart": false}));
        assert_eq!(daemon_json(app.handle_key(ch('a'))), json!({"cmd": "autostart", "enabled": true}));
        app.status = Ok(json!({"terminal": {"on": false}}));
        assert_eq!(daemon_json(app.handle_key(ch('p'))), json!({"cmd": "shell_path", "enabled": true}), "p adds");
        app.status = Ok(json!({"terminal": {"on": true}}));
        assert_eq!(daemon_json(app.handle_key(ch('p'))), json!({"cmd": "shell_path", "enabled": false}));
    }

    #[test]
    fn mail_page() {
        let mut app = App::new(false);
        app.page = Page::Mail;
        app.status = Ok(json!({"mail": {"enabled": false, "state": "off", "port": 2525}}));
        let on =
            json!({"cmd": "mail_set", "enabled": true, "port": null, "keep": null, "php_mail": null, "notify": null});
        assert_eq!(daemon_json(app.handle_key(ch('m'))), on, "m turns Mail on");
        assert!(matches!(app.handle_key(ch('t')), Action::None));
        assert_eq!(app.message.as_ref().map(|m| m.0.as_str()), Some("Mail is off; m turns it on"));
        assert!(matches!(app.handle_key(ch('c')), Action::Do(Effect::Copy(env)) if env.contains("MAIL_PORT=2525")));
        app.status = Ok(json!({"mail": {"enabled": true, "state": "running", "port": 2525,
            "inbox_url": "http://127.0.0.1:2526/", "php_mail": true}}));
        assert_eq!(daemon_json(app.handle_key(ch('t'))), json!({"cmd": "mail_test"}));
        assert!(matches!(app.handle_key(ch('o')), Action::Do(Effect::OpenUrl(u)) if u == "http://127.0.0.1:2526/"));
        assert_eq!(daemon_json(app.handle_key(ch('p')))["php_mail"], false, "p toggles PHP's mail()");
        app.handle_key(ch('X'));
        assert_eq!(daemon_json(app.handle_key(ch('y'))), json!({"cmd": "mail_clear"}));
        assert_eq!(daemon_json(app.handle_key(ch('m')))["enabled"], false, "m turns a running Mail off");
        app.handle_key(ch('7'));
        assert_eq!(app.page, Page::Settings, "Databases is page 5, so Settings is 7");
    }

    fn with_databases() -> App {
        let mut app = App::new(false);
        app.page = Page::Databases;
        app.focus = Focus::Page;
        app.status = Ok(json!({"databases": [
            {"engine": "mysql", "label": "MySQL", "series": "8.4", "enabled": true, "state": "running",
             "port": 3307, "installed": ["8.4"]},
            {"engine": "postgresql", "label": "PostgreSQL", "series": null, "enabled": false, "state": "off",
             "port": null, "installed": []},
        ]}));
        app
    }

    /// D-061: start and stop a database, and its main account's `.env` lines.
    #[test]
    fn databases_page() {
        let mut app = with_databases();
        assert_eq!(app.rows(), 2);
        let keys = |d: &Value| App::db_menu(d).iter().map(|(k, _)| *k).collect::<String>();
        assert_eq!(keys(&app.databases()[0]), "acnors");
        assert_eq!(keys(&app.databases()[1]), "s", "an engine that is off only starts");
        assert!(App::db_menu(&app.databases()[1])[0].1.contains("downloads it"));

        app.handle_key(key(KeyCode::Enter));
        assert!(matches!(app.handle_key(ch('c')), Action::Do(Effect::DbCopy { name: None, password: false, .. })));
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(ch('s'));
        assert!(matches!(app.mode, Mode::Confirm { .. }), "stopping asks first");
        assert_eq!(daemon_json(app.handle_key(ch('y'))), json!({"cmd": "db_stop", "engine": "mysql"}));
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(ch('o'));
        assert!(matches!(&app.mode, Mode::Input { text, .. } if text == "3307"), "the port to change");
        app.handle_key(key(KeyCode::Backspace));
        app.handle_key(ch('8'));
        assert_eq!(
            daemon_json(app.handle_key(key(KeyCode::Enter))),
            json!({"cmd": "db_port", "engine": "mysql", "port": 3308})
        );
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(ch('n'));
        for c in "blog".chars() {
            app.handle_key(ch(c));
        }
        assert_eq!(
            daemon_json(app.handle_key(key(KeyCode::Enter))),
            json!({"cmd": "db_create", "engine": "mysql", "database": "blog"})
        );

        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(
            daemon_json(app.handle_key(ch('s'))),
            json!({"cmd": "db_start", "engine": "postgresql", "series": null})
        );
    }

    /// D-061: add accounts, give one a new password, copy, remove; the main account stays.
    #[test]
    fn accounts_view() {
        let mut app = with_databases();
        app.handle_key(key(KeyCode::Enter));
        assert!(matches!(app.handle_key(ch('a')), Action::Do(Effect::LoadAccounts(e)) if e == "mysql"));
        app.open_accounts(&json!({"engine": "mysql", "series": "8.4", "port": 3307, "users": [
            {"name": "root", "database": null, "main": true, "password": "none"},
            {"name": "shop", "database": "shop", "main": false, "password": "kept"},
        ]}));
        assert!(matches!(&app.mode, Mode::Accounts(v) if v.label == "MySQL" && v.rows.len() == 2));

        app.handle_key(ch('d'));
        assert!(matches!(app.mode, Mode::Accounts(_)), "the main account stays");
        assert!(app.message.as_ref().is_some_and(|m| m.0.contains("main account")));

        app.handle_key(ch('j'));
        assert!(matches!(
            app.handle_key(ch('y')),
            Action::Do(Effect::DbCopy { name: Some(n), password: true, .. }) if n == "shop"
        ));
        assert!(matches!(app.mode, Mode::Accounts(_)), "copying keeps the view open");
        app.handle_key(ch('p'));
        assert!(matches!(app.mode, Mode::ConfirmAccount { .. }), "a new password asks first");
        app.handle_key(ch('n'));
        assert!(matches!(&app.mode, Mode::Accounts(v) if v.index == 1), "no goes back to the view");
        app.handle_key(ch('p'));
        match app.handle_key(ch('y')) {
            Action::Do(Effect::DbAccount(r)) => assert_eq!(
                serde_json::to_value(r).unwrap(),
                json!({"cmd": "db_user_password", "engine": "mysql", "name": "shop", "password": null})
            ),
            other => panic!("expected a new password, got {other:?}"),
        }
        app.open_accounts(&json!({"engine": "mysql", "series": "8.4", "users": [
            {"name": "shop", "database": "shop", "main": false, "password": "kept"}]}));
        app.handle_key(ch('d'));
        assert_eq!(
            daemon_json(app.handle_key(ch('y'))),
            json!({"cmd": "db_user_remove", "engine": "mysql", "name": "shop"})
        );
        assert_eq!(app.reopen_accounts.as_deref(), Some("mysql"), "the view comes back after a removal");

        app.open_accounts(&json!({"engine": "mysql", "series": "8.4", "users": []}));
        app.handle_key(ch('a'));
        for c in "blog blogdb".chars() {
            app.handle_key(ch(c));
        }
        match app.handle_key(key(KeyCode::Enter)) {
            Action::Do(Effect::DbAccount(r)) => assert_eq!(
                serde_json::to_value(r).unwrap(),
                json!({"cmd": "db_user_add", "engine": "mysql", "name": "blog", "database": "blogdb",
                       "password": null, "no_database": false, "create_databases": false})
            ),
            other => panic!("expected a new account, got {other:?}"),
        }
        // Generated, so no password-like literal sits in the source for secret scanners.
        let generated = devid_daemon::db::password();
        app.open_secret(
            &json!({"engine": "mysql", "name": "blog", "database": "blogdb", "password": generated,
                    "env": ["DB_CONNECTION=mysql", "DB_USERNAME=blog"]}),
            true,
        );
        assert!(matches!(&app.mode, Mode::Secret(s) if s.title.contains("with the database blogdb")));
        assert!(matches!(
            app.handle_key(ch('c')),
            Action::Do(Effect::CopyQuiet { text, .. }) if text == "DB_CONNECTION=mysql\nDB_USERNAME=blog\n"
        ));
        assert!(matches!(app.handle_key(ch('y')), Action::Do(Effect::CopyQuiet { text, .. }) if text == generated));
        assert!(matches!(app.handle_key(key(KeyCode::Esc)), Action::Do(Effect::LoadAccounts(_))));
    }

    #[test]
    fn new_project_wizard() {
        let mut app = with_sites();
        app.handle_key(ch('n'));
        let Mode::NewProject(v) = &app.mode else { panic!("wizard opens on n") };
        assert_eq!(v.template().key, "laravel");
        assert_eq!(v.fields(), [NewField::Template, NewField::Name, NewField::Kit, NewField::Db, NewField::Create]);
        // An empty name is refused at Create, and focus goes to the name.
        for _ in 0..4 {
            app.handle_key(key(KeyCode::Down));
        }
        app.handle_key(key(KeyCode::Enter));
        let Mode::NewProject(v) = &app.mode else { panic!("still open") };
        assert!(v.error.as_deref().is_some_and(|e| e.contains("not a project name")));
        assert_eq!(v.field, NewField::Name);
        for c in "My-Shop!".chars() {
            app.handle_key(ch(c));
        }
        app.handle_key(key(KeyCode::Down)); // kit
        app.handle_key(key(KeyCode::Right)); // react
        app.handle_key(key(KeyCode::Down)); // db
        app.handle_key(key(KeyCode::Right)); // mysql
        app.handle_key(key(KeyCode::Down));
        let Action::Do(Effect::NewProject(v)) = app.handle_key(key(KeyCode::Enter)) else { panic!("create") };
        assert_eq!(
            v.request(),
            Request::NewProject {
                template: "laravel".into(),
                name: "my-shop".into(),
                path: None,
                php: None,
                db: Some("mysql".into()),
                kit: Some("react".into()),
            }
        );
        // Symfony's skeleton has no database field; Node.js templates have neither.
        let mut v =
            NewView { template: TEMPLATES.iter().position(|t| t.key == "symfony").unwrap(), ..Default::default() };
        assert!(v.fields().contains(&NewField::Db));
        v.kit = 1;
        assert!(!v.fields().contains(&NewField::Db) && v.db().is_none());
        assert_eq!(
            v.request(),
            Request::NewProject {
                template: "symfony".into(),
                name: String::new(),
                path: None,
                php: None,
                db: None,
                kit: Some("skeleton".into()),
            }
        );
        let v = NewView { template: TEMPLATES.iter().position(|t| t.key == "next").unwrap(), ..Default::default() };
        assert_eq!(v.fields(), [NewField::Template, NewField::Name, NewField::Create]);
        // ← on the template wraps around the list; esc closes.
        let mut app = with_sites();
        app.handle_key(ch('n'));
        for _ in 0..TEMPLATES.len() {
            app.handle_key(key(KeyCode::Left));
        }
        let Mode::NewProject(v) = &app.mode else { panic!() };
        assert_eq!(v.template().key, "laravel");
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);
        // What was made, and its keys.
        app.open_new_done(&serde_json::json!({"name": "shop", "url": "https://shop.test", "path": "/x/shop",
            "php": "8.5", "port": null, "account": null, "admin": {"user": "admin", "password": "pw"},
            "commands": [], "notes": [], "log": "/x/new-shop.log"}));
        let Mode::NewDone { title, lines, .. } = &app.mode else { panic!() };
        assert_eq!(title, "shop is ready");
        assert!(lines.iter().any(|l| l.contains("admin / pw")));
        assert!(matches!(app.handle_key(ch('o')), Action::Do(Effect::OpenUrl(u)) if u == "https://shop.test"));
        assert!(matches!(app.handle_key(key(KeyCode::Esc)), Action::Refresh));
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
