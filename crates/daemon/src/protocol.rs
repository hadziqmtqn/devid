//! Wire format: one JSON object per line in each direction.
//! Requests are typed; replies are JSON objects with `ok: true` or `ok: false, error`.
//! Paths are absolute; the CLI resolves them before sending.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Status,
    Stop,
    PhpList,
    /// Install a PHP version from a build folder, or from the release manifest when `from` is None.
    PhpInstall {
        version: String,
        from: Option<String>,
    },
    PhpUse {
        version: String,
    },
    /// Turn xdebug or pcov on or off for a PHP version (restarts that version's php-fpm).
    PhpExt {
        version: String,
        ext: String,
        enabled: bool,
    },
    /// Catalog extensions (Phase 6) the release offers for a PHP version, marked installed or not.
    PhpExtCatalog {
        version: String,
    },
    /// Download, verify and turn on catalog extensions, one after another; php-fpm restarts once at
    /// the end. Replies with one result per name.
    PhpExtInstall {
        version: String,
        names: Vec<String>,
    },
    /// The running `PhpExtInstall`, if any: which extension, phase and percent.
    PhpExtProgress,
    PhpExtRemove {
        version: String,
        name: String,
    },
    /// Bundled extensions (`php -m`), xdebug/pcov on or off, and installed catalog extensions.
    PhpExtList {
        version: String,
    },
    /// Restart one service: "php-8.4", "nginx", "caddy" or "apache".
    Restart {
        service: String,
    },
    /// Stop a service and keep it stopped until `ServiceStart` (or a daemon restart).
    ServiceStop {
        service: String,
    },
    ServiceStart {
        service: String,
    },
    StopAll,
    StartAll,
    /// Log files in ~/.devid/log.
    Logs,
    /// A log's tail (offset None) or what is new since `offset`; replies with the new offset.
    LogRead {
        name: String,
        offset: Option<u64>,
    },
    LogClear {
        name: String,
    },
    /// Install a web server binary (`from` optional for apache: the system httpd).
    ServerInstall {
        server: String,
        from: Option<String>,
    },
    /// Make an installed web server the active one (D-005: one at a time).
    ServerUse {
        server: String,
    },
    ServerList,
    /// Compare installed PHP/nginx/Caddy with the release manifest; install newer ones unless `check`.
    Update {
        check: bool,
    },
    /// How the running (or the last) `update` that installs is going.
    UpdateProgress,
    Sites,
    /// Serve `path` as `<name>.<tld>`; the name defaults to the folder name.
    Link {
        name: Option<String>,
        path: String,
    },
    /// What `link` would make of `path`, without saving it: the name, type, mode, dev server port
    /// and PHP version, and `taken`, the path of a site that already has that name.
    LinkPreview {
        name: Option<String>,
        path: String,
    },
    Unlink {
        name: String,
    },
    /// Serve every subfolder of `path`.
    Park {
        path: String,
    },
    Unpark {
        path: String,
    },
    /// Stop serving a parked site; its folder is not touched (D-029).
    Ignore {
        site: String,
    },
    /// Serve an ignored parked site again.
    Unignore {
        site: String,
    },
    /// Start nginx and php-fpm at login, or wait for `start all` (D-031).
    Autostart {
        enabled: bool,
    },
    /// Open the desktop app at login (D-031). `app` is its binary; default: ~/Applications/Devid.app.
    LoginItem {
        enabled: bool,
        app: Option<String>,
    },
    /// Put `~/.devid/bin` (devid, php, php8.4, composer, nginx, caddy, mailpit…) on PATH in the
    /// login shell's startup files, or take it out (D-053). `status` reports it as `terminal`.
    ShellPath {
        enabled: bool,
    },
    /// Pin a site to a PHP version; the site is `site` or the one containing `cwd`.
    Isolate {
        version: String,
        site: Option<String>,
        cwd: String,
    },
    Unisolate {
        site: Option<String>,
        cwd: String,
    },
    /// Change how a site is served (D-035); the site is `site` or the one containing `cwd`. Only the
    /// given fields change. A new `preset` without `mode` drops the old mode, root and upstream,
    /// so the preset's defaults apply.
    AppSet {
        site: Option<String>,
        cwd: String,
        #[serde(default)]
        preset: Option<String>,
        #[serde(default)]
        mode: Option<String>,
        #[serde(default)]
        root: Option<String>,
        #[serde(default)]
        upstream: Option<u16>,
        #[serde(default)]
        spa: Option<bool>,
        #[serde(default)]
        pass_host: Option<bool>,
        /// D-054: the saved command a request to this proxy site starts, or "" to turn it off.
        #[serde(default)]
        on_demand: Option<String>,
        /// D-054: minutes without use before the on-demand command stops; 0 never.
        #[serde(default)]
        idle_stop: Option<u32>,
    },
    /// Forget a site's app overrides: the detected type applies again. The local port, the
    /// commands and the on-demand setting stay.
    AppReset {
        site: Option<String>,
        cwd: String,
    },
    /// Also serve a site on http://127.0.0.1:<port> (D-037). `port`: None keeps the site's port or
    /// picks the first free one from 8000; Some(0) turns it off. Sent again, it checks a busy port anew.
    AppPort {
        site: Option<String>,
        cwd: String,
        port: Option<u16>,
    },
    /// Save a command for a site (D-036): `name` is how it is shown and logged, `run` a shell line
    /// run in the site's folder. Saving a name again replaces its line.
    CommandAdd {
        site: Option<String>,
        cwd: String,
        name: String,
        run: String,
    },
    /// Forget a site's command; a running one is stopped first.
    CommandRemove {
        site: Option<String>,
        cwd: String,
        name: String,
    },
    /// Start one of a site's commands, or all of them when `name` is None. Replies with one result
    /// per command.
    CommandStart {
        site: Option<String>,
        cwd: String,
        name: Option<String>,
    },
    /// Stop one of a site's commands, or all of them when `name` is None, and wait until nothing of
    /// them is left.
    CommandStop {
        site: Option<String>,
        cwd: String,
        name: Option<String>,
    },
    /// Share a site through a Cloudflare quick tunnel (D-037); replies with its trycloudflare.com
    /// URL, or none yet if cloudflared is still connecting. Sent again, it only reports the URL.
    ShareStart {
        site: Option<String>,
        cwd: String,
    },
    /// Stop sharing a site.
    ShareStop {
        site: Option<String>,
        cwd: String,
    },
    /// Before a restart Devid does itself (`devid setup`, also run by `self-update`): note the
    /// running commands so the next daemon starts them again (D-040). Not shares.
    RememberRunning,
    /// Mail settings (D-049); a field left out stays as it is. The first time Mail is turned on,
    /// Mailpit is downloaded from the release.
    MailSet {
        #[serde(default)]
        enabled: Option<bool>,
        #[serde(default)]
        port: Option<u16>,
        #[serde(default)]
        keep: Option<u32>,
        #[serde(default)]
        php_mail: Option<bool>,
        #[serde(default)]
        notify: Option<bool>,
    },
    /// Send a test message to Mailpit over SMTP.
    MailTest,
    /// Delete every message in Mailpit.
    MailClear,
    /// The newest messages, or those matching `query` (Mailpit's search: words, from:, to:, subject:,
    /// has:attachment…); `unread` keeps only unread ones.
    MailList {
        #[serde(default)]
        query: String,
        #[serde(default)]
        unread: bool,
        #[serde(default)]
        limit: Option<u32>,
    },
    /// One message: sender, recipients, subject, date, text, HTML and attachments. Mailpit marks it read.
    MailMessage {
        id: String,
    },
    /// One message's headers.
    MailHeaders {
        id: String,
    },
    /// One message's source as it arrived (cut at 512 KB).
    MailRaw {
        id: String,
    },
    /// Mark messages read or unread; no IDs: all of them.
    MailRead {
        #[serde(default)]
        ids: Vec<String>,
        read: bool,
    },
    /// Delete messages (`MailClear` deletes all).
    MailDelete {
        ids: Vec<String>,
    },
    /// Save one attachment to ~/Downloads; the reply has its path.
    MailSave {
        id: String,
        part: String,
    },
    /// Databases (D-061): each engine's server and installed series. With `available`, also the
    /// series the database release offers for this computer (it downloads the database manifest).
    DbList {
        #[serde(default)]
        available: bool,
    },
    /// Download a series' newest build and put it in place, without starting it; MongoDB brings
    /// mongosh along.
    DbInstall {
        engine: String,
        #[serde(default)]
        series: Option<String>,
    },
    /// Run an engine's server from now on, in `series` (default: the one in use, else the engine's
    /// default): installs and sets it up first if needed, and picks its port at the first start.
    DbStart {
        engine: String,
        #[serde(default)]
        series: Option<String>,
    },
    /// Stop an engine's server and keep it off; its data stays.
    DbStop {
        engine: String,
    },
    /// Make an account with a generated password (or `password`), and by default a database of its
    /// own named after it (`database`, or none with `no_database`; MongoDB always has one). The
    /// reply has the password and the `.env` lines.
    DbUserAdd {
        engine: String,
        name: String,
        #[serde(default)]
        database: Option<String>,
        #[serde(default)]
        password: Option<String>,
        #[serde(default)]
        no_database: bool,
    },
    /// Give an account a new password: a generated one, or `password`.
    DbUserPassword {
        engine: String,
        name: String,
        #[serde(default)]
        password: Option<String>,
    },
    /// The accounts people sign in with, and whether Devid knows each one's password.
    DbUserList {
        engine: String,
    },
    /// Remove an account; its databases stay.
    DbUserRemove {
        engine: String,
        name: String,
    },
    /// The `.env` lines for an account (default: the main one).
    DbEnv {
        engine: String,
        #[serde(default)]
        name: Option<String>,
    },
    /// A database without an account of its own.
    DbCreate {
        engine: String,
        database: String,
    },
    /// Move a server to another port; it restarts there.
    DbPort {
        engine: String,
        port: u16,
    },
    /// Remove an installed series that is not in use; its data stays unless `delete_data`.
    DbRemove {
        engine: String,
        series: String,
        #[serde(default)]
        delete_data: bool,
    },
    /// D-063: the templates `devid new` offers, with what each needs that this computer lacks.
    NewTemplates,
    /// D-063: make a project with its framework's own tool, then serve it at `<name>.<tld>`. `path`
    /// is the project's folder (default: `<first parked folder>/<name>`); `php` pins the site to a
    /// version. Replies when the job ends; `NewProjectProgress` shows its steps meanwhile.
    NewProject {
        template: String,
        name: String,
        #[serde(default)]
        path: Option<String>,
        #[serde(default)]
        php: Option<String>,
        #[serde(default)]
        db: Option<String>,
        #[serde(default)]
        kit: Option<String>,
    },
    /// The running (or the last) `NewProject`: its steps, and how it ended.
    NewProjectProgress,
}

/// Send one request to the daemon's socket and return its JSON reply.
/// Shared by every client: the CLI, the TUI and the desktop app.
pub fn request(paths: &crate::paths::Paths, req: &Request) -> Result<serde_json::Value, String> {
    use std::io::{BufRead, BufReader, Write};
    let sock = paths.socket();
    let mut stream = std::os::unix::net::UnixStream::connect(&sock).map_err(|_| {
        format!("daemon is not running (no socket at {}); start it with `devid daemon run`", sock.display())
    })?;
    let mut line = serde_json::to_string(req).expect("request serializes");
    line.push('\n');
    stream.write_all(line.as_bytes()).map_err(|e| e.to_string())?;

    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).map_err(|e| e.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&reply).map_err(|e| format!("bad reply from daemon: {e}"))?;
    if value["ok"] == false {
        return Err(value["error"].as_str().unwrap_or("daemon returned an error").to_string());
    }
    Ok(value)
}
