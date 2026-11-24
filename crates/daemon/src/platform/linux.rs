//! Linux (D-057, D-059): systemd-resolved for `.<tld>`, an nftables table loaded by a system unit
//! for ports 80/443, the system CA store, `certutil` for the browsers' own databases, and a systemd
//! user unit for the daemon. Ubuntu came first; Arch differs in two parts, and each is picked by
//! what the system has (`System`), not by its name:
//! - the CA store: Debian's `update-ca-certificates`, or p11-kit's anchors and `update-ca-trust`;
//! - DNS: where systemd-resolved is off, the root step turns it on, points /etc/resolv.conf at its
//!   stub and restarts NetworkManager, and uninstall puts each back (Phase 13 A1).

use super::Check;
use crate::{
    ca,
    helper::{Action, Plan},
    install,
    paths::{Paths, user_home},
    php, phpext,
};
use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub const NAME: &str = "Linux";
pub const OPEN_COMMAND: &str = "xdg-open";
/// Wayland's, then X11's clipboard tools; the first that runs wins.
pub const COPY_COMMANDS: &[&[&str]] =
    &[&["wl-copy"], &["xclip", "-selection", "clipboard"], &["xsel", "--clipboard", "--input"]];
pub const REDIRECT_FIX: &str =
    "the nftables redirect is not loaded (a firewall tool may have flushed the ruleset); run `devid helper install`";
pub const SERVICE: &str = "systemd user unit devid.service";
/// The desktop app comes to Linux after the CLI and the TUI (D-057).
pub const DESKTOP_APP: bool = false;

pub const RESOLVED_DROPIN: &str = "/etc/systemd/resolved.conf.d/devid.conf";
/// Written when the root step turns systemd-resolved on, so uninstall knows to turn it off again.
pub const RESOLVED_STATE: &str = "/etc/devid-resolved.state";
pub const RESOLV_CONF: &str = "/etc/resolv.conf";
/// systemd-resolved's stub, relative as in systemd's own tmpfiles.d link.
const STUB: &str = "../run/systemd/resolve/stub-resolv.conf";
pub const NFT_FILE: &str = "/etc/devid-ports.nft";
pub const PORTS_UNIT: &str = "/etc/systemd/system/devid-ports.service";
/// The CA's copy for Debian's `update-ca-certificates`.
pub const CA_FILE: &str = "/usr/local/share/ca-certificates/devid-ca.crt";
/// The CA's copy for p11-kit's `update-ca-trust` (Arch).
pub const CA_ANCHOR: &str = "/etc/ca-certificates/trust-source/anchors/devid-ca.crt";
pub const CA_BUNDLE: &str = "/etc/ssl/certs/ca-certificates.crt";
const SYSTEMCTL: &str = "/usr/bin/systemctl";
const NFT: &str = "/usr/sbin/nft";
const UPDATE_CA: &str = "/usr/sbin/update-ca-certificates";
const UPDATE_CA_TRUST: &str = "/usr/bin/update-ca-trust";
const TRUST_SOURCE: &str = "/etc/ca-certificates/trust-source";
const PACMAN: &str = "/usr/bin/pacman";
const UNIT: &str = "devid.service";
const NSS_NICKNAME: &str = "Devid Local CA";
const RESOLVED_STATE_NOTE: &str = "# Devid turned systemd-resolved on and pointed /etc/resolv.conf at its stub (D-059).\n\
     # `devid uninstall` turns it off again and puts back /etc/resolv.conf.devid-backup.\n";

/// What the root step depends on (D-059): read from the system by `detect`, or set by a test.
#[derive(Debug, Clone, PartialEq)]
pub struct System {
    /// The CA store is p11-kit's (Arch), where Debian's `update-ca-certificates` is missing.
    pub p11kit: bool,
    /// systemd-resolved runs: on Ubuntu and CachyOS by default, not on a stock Arch desktop (A1).
    pub resolved_active: bool,
    /// The root step turned systemd-resolved on earlier (RESOLVED_STATE).
    pub resolved_ours: bool,
    /// The sockets that start systemd-resolved. They stop with it, or the next lookup through
    /// nss-resolve starts it again (A1).
    pub resolved_sockets: Vec<String>,
    /// /etc/resolv.conf.devid-backup exists.
    pub resolv_backup: bool,
    /// /etc/resolv.conf is a link to systemd-resolved's stub.
    pub resolv_stub: bool,
}

fn systemctl_ok(args: &[&str]) -> bool {
    Command::new(SYSTEMCTL).args(args).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

impl System {
    pub fn detect() -> System {
        let sockets = Command::new(SYSTEMCTL)
            .args(["show", "--property=TriggeredBy", "--value", "systemd-resolved.service"])
            .stderr(Stdio::null())
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).split_whitespace().map(String::from).collect())
            .unwrap_or_default();
        System {
            p11kit: !Path::new(UPDATE_CA).exists()
                && Path::new(UPDATE_CA_TRUST).exists()
                && Path::new(TRUST_SOURCE).is_dir(),
            resolved_active: systemctl_ok(&["is-active", "--quiet", "systemd-resolved"]),
            resolved_ours: Path::new(RESOLVED_STATE).exists(),
            resolved_sockets: sockets,
            resolv_backup: crate::helper::present(&crate::helper::backup_path(Path::new(RESOLV_CONF))),
            resolv_stub: fs::read_link(RESOLV_CONF).is_ok_and(|t| t.ends_with("systemd/resolve/stub-resolv.conf")),
        }
    }

    /// Where the CA's copy goes.
    fn ca_copy(&self) -> &'static str {
        if self.p11kit { CA_ANCHOR } else { CA_FILE }
    }

    /// The command that rebuilds the system bundle from the copies, after adding or removing one.
    fn update_ca(&self, removing: bool) -> Action {
        match (self.p11kit, removing) {
            (true, _) => run(UPDATE_CA_TRUST, &["extract"], removing),
            (false, false) => run(UPDATE_CA, &[], false),
            (false, true) => run(UPDATE_CA, &["--fresh"], true),
        }
    }
}

/// Whether pacman is the package manager (Arch and its derivatives), for the install commands in
/// messages; apt otherwise.
fn pacman() -> bool {
    Path::new(PACMAN).exists()
}

/// The command that installs packages: `apt` names for Debian and Ubuntu, `arch` names for pacman.
fn install_hint(pacman: bool, apt: &str, arch: &str) -> String {
    if pacman { format!("`sudo pacman -S --needed {arch}`") } else { format!("`sudo apt install {apt}`") }
}

fn no_certutil(pacman: bool) -> String {
    format!(
        "Chrome and Firefox do not trust https://*.test yet: install certutil ({}), then run `devid setup` again",
        install_hint(pacman, "libnss3-tools", "nss")
    )
}

/// The Linux devid binary in a devid release (self-update, install.sh): a static musl build (L4).
pub const DEVID_ASSET: &str = if cfg!(target_arch = "aarch64") { "devid-linux-arm64" } else { "devid-linux-x86_64" };

/// The arch key of Linux builds in the runtime manifest, apart from the macOS keys (L2).
pub fn arch_key() -> &'static str {
    if cfg!(target_arch = "aarch64") { "linux-arm64" } else { "linux-x86_64" }
}

/// A global drop-in: systemd-resolved sends `.<tld>` to Devid's DNS port and nothing else. L1 found
/// that a link of Devid's own gets no DNS scope without an address.
pub fn resolved_dropin(plan: &Plan) -> String {
    format!(
        "# Devid: send .{tld} lookups to the Devid daemon, and nothing else\n[Resolve]\nDNS=127.0.0.1:{port}\nDomains=~{tld}\n",
        tld = plan.tld,
        port = plan.dns_port
    )
}

/// The pf anchor's twin: 127.0.0.1 ports 80 and 443 go to the web server's high ports. Loading it
/// replaces an earlier copy, since it deletes the table before adding it again.
pub fn nft_rules(plan: &Plan) -> String {
    format!(
        "# Devid: local-only redirect of the standard web ports to the web server's high ports\n\
         table inet devid\n\
         delete table inet devid\n\
         table inet devid {{\n\
         \x20 chain output {{\n\
         \x20   type nat hook output priority -100; policy accept;\n\
         \x20   ip daddr 127.0.0.1 tcp dport 80 redirect to :{}\n\
         \x20   ip daddr 127.0.0.1 tcp dport 443 redirect to :{}\n\
         \x20 }}\n\
         }}\n",
        plan.http_port, plan.https_port
    )
}

/// Loads the table at boot, and deletes it when stopped.
pub fn ports_unit() -> String {
    format!(
        "[Unit]\n\
         Description=Devid: send 127.0.0.1 ports 80 and 443 to Devid's web server\n\
         # After nftables.service, whose /etc/nftables.conf starts with `flush ruleset`.\n\
         After=network-pre.target nftables.service\n\
         \n\
         [Service]\n\
         Type=oneshot\n\
         RemainAfterExit=yes\n\
         ExecStart={NFT} -f {NFT_FILE}\n\
         ExecStop={NFT} delete table inet devid\n\
         \n\
         [Install]\n\
         WantedBy=multi-user.target\n"
    )
}

fn run(program: &str, args: &[&str], ok_to_fail: bool) -> Action {
    Action::Run { program: program.into(), args: args.iter().map(|s| s.to_string()).collect(), ok_to_fail }
}

pub fn install_actions(plan: &Plan) -> Vec<Action> {
    install_actions_on(plan, &System::detect())
}

pub fn install_actions_on(plan: &Plan, sys: &System) -> Vec<Action> {
    let turn_on = !sys.resolved_active;
    let mut actions = Vec::new();
    if turn_on {
        // First: if a later step fails, uninstall still turns systemd-resolved off again.
        actions.push(Action::Write { path: RESOLVED_STATE.into(), content: RESOLVED_STATE_NOTE.into(), backup: false });
    }
    actions.extend([
        Action::Write { path: RESOLVED_DROPIN.into(), content: resolved_dropin(plan), backup: true },
        Action::Write { path: NFT_FILE.into(), content: nft_rules(plan), backup: false },
        Action::Write { path: PORTS_UNIT.into(), content: ports_unit(), backup: false },
        // Before any restart: systemctl warns about unit files changed on disk otherwise, as after
        // the Ubuntu 24.04 machine's upgrade (0.19.0-rc.1).
        run(SYSTEMCTL, &["daemon-reload"], false),
    ]);
    if turn_on {
        actions.push(run(SYSTEMCTL, &["enable", "systemd-resolved.service"], false));
    }
    actions.push(run(SYSTEMCTL, &["restart", "systemd-resolved"], false));
    if turn_on {
        // NetworkManager hands its own servers to systemd-resolved once /etc/resolv.conf points at
        // the stub, which it checks when it starts (A1).
        actions.push(Action::Link { path: RESOLV_CONF.into(), target: STUB.into() });
        actions.push(run(SYSTEMCTL, &["try-restart", "NetworkManager.service"], false));
    }
    actions.extend([
        run(SYSTEMCTL, &["enable", "devid-ports.service"], false),
        // restart, not start: a running unit must load changed ports.
        run(SYSTEMCTL, &["restart", "devid-ports.service"], false),
        Action::Copy { from: plan.ca_cert.clone(), to: sys.ca_copy().into() },
        sys.update_ca(false),
    ]);
    actions
}

pub fn uninstall_actions(plan: &Plan) -> Vec<Action> {
    uninstall_actions_on(plan, &System::detect())
}

pub fn uninstall_actions_on(_plan: &Plan, sys: &System) -> Vec<Action> {
    let mut actions = vec![
        // Stopping runs the unit's ExecStop, which deletes the table.
        run(SYSTEMCTL, &["disable", "--now", "devid-ports.service"], true),
        Action::Remove { path: PORTS_UNIT.into(), restore_backup: false },
        run(SYSTEMCTL, &["daemon-reload"], true),
        Action::Remove { path: NFT_FILE.into(), restore_backup: false },
        Action::Remove { path: RESOLVED_DROPIN.into(), restore_backup: true },
    ];
    if sys.resolved_ours {
        // Back to how setup found it (A1). /etc/resolv.conf is never left missing: at the next boot
        // systemd's tmpfiles.d would link a missing one to the stub of a resolver that is off.
        if sys.resolv_backup {
            actions.push(Action::Restore { path: RESOLV_CONF.into() });
        } else if sys.resolv_stub {
            // Nothing was there before Devid's link: NetworkManager writes a new file when it restarts.
            actions.push(Action::Remove { path: RESOLV_CONF.into(), restore_backup: false });
        }
        let mut args: Vec<String> = ["disable", "--now", "systemd-resolved.service"].map(String::from).into();
        args.extend(sys.resolved_sockets.iter().cloned());
        actions.push(Action::Run { program: SYSTEMCTL.into(), args, ok_to_fail: true });
        actions.push(run(SYSTEMCTL, &["try-restart", "NetworkManager.service"], true));
        actions.push(Action::Remove { path: RESOLVED_STATE.into(), restore_backup: false });
    } else {
        actions.push(run(SYSTEMCTL, &["restart", "systemd-resolved"], true));
    }
    actions.push(Action::Remove { path: sys.ca_copy().into(), restore_backup: false });
    actions.push(sys.update_ca(true));
    actions
}

/// The base64 body of a PEM certificate, without its armor or line breaks, for comparing copies.
fn pem_body(pem: &str) -> String {
    pem.lines().filter(|l| !l.starts_with("-----")).flat_map(|l| l.split_whitespace()).collect()
}

/// Whether the system bundle holds the CA: `update-ca-certificates` ran after the copy.
fn bundle_has(bundle: &str, ca_pem: &str) -> bool {
    let body = pem_body(ca_pem);
    !body.is_empty() && pem_body(bundle).contains(&body)
}

pub fn system_status(plan: &Plan) -> Vec<(&'static str, bool)> {
    let sys = System::detect();
    // The drop-in does nothing while systemd-resolved is off (a stock Arch desktop, A1).
    let resolver = sys.resolved_active && fs::read_to_string(RESOLVED_DROPIN).is_ok_and(|c| c == resolved_dropin(plan));
    let ports = redirect_installed(plan) && fs::read_to_string(PORTS_UNIT).is_ok_and(|c| c == ports_unit());
    let trusted = match (fs::read_to_string(&plan.ca_cert), fs::read_to_string(sys.ca_copy())) {
        (Ok(ca), Ok(copy)) => ca == copy && fs::read_to_string(CA_BUNDLE).is_ok_and(|b| bundle_has(&b, &ca)),
        _ => false,
    };
    vec![("resolver", resolver), ("ports 80/443", ports), ("CA trusted", trusted)]
}

/// Whether the redirect for these ports is installed, so URLs need no port.
pub fn redirect_installed(plan: &Plan) -> bool {
    fs::read_to_string(NFT_FILE).is_ok_and(|c| c == nft_rules(plan))
}

/// What the root step needs, checked before `sudo`: the tools, and a resolver it can use.
pub fn prerequisites() -> Result<(), String> {
    prerequisites_on(
        |path| Path::new(path).exists(),
        systemctl_ok(&["is-active", "--quiet", "systemd-resolved"]),
        systemctl_ok(&["is-active", "--quiet", "NetworkManager"]),
    )
}

fn prerequisites_on(exists: impl Fn(&str) -> bool, resolved: bool, network_manager: bool) -> Result<(), String> {
    let ca_tool = exists(UPDATE_CA) || (exists(UPDATE_CA_TRUST) && exists(TRUST_SOURCE));
    let missing: Vec<&str> = [(exists(SYSTEMCTL), "systemd"), (exists(NFT), "nftables"), (ca_tool, "ca-certificates")]
        .into_iter()
        .filter(|(there, _)| !there)
        .map(|(_, package)| package)
        .collect();
    if !missing.is_empty() {
        let packages = missing.join(" ");
        return Err(format!(
            "the system setup needs {packages}: {}",
            install_hint(exists(PACMAN), &packages, &packages)
        ));
    }
    // With systemd-resolved off, setup turns it on (D-059); NetworkManager then hands it the
    // network's own servers. Without NetworkManager nothing would.
    if !resolved && !network_manager {
        return Err("Devid sends .test lookups through systemd-resolved (D-057), which does not run here, and \
             setup turns it on only next to NetworkManager (D-059). Turn systemd-resolved on and point \
             /etc/resolv.conf at its stub, then run `devid setup` again"
            .into());
    }
    Ok(())
}

/// The startup files that put `~/.devid/bin` on PATH (D-053), for the login shell:
/// - bash (the default): ~/.bashrc, which the terminal's non-login shells read; and the file a login
///   shell reads, which is how the desktop session starts, so editors opened from the menu find
///   `php`. That is ~/.bash_profile where it exists (Arch's /etc/skel has one), else ~/.profile
///   (Debian and Ubuntu). On Arch, SDDM starts Plasma through `bash --login`, which reads only
///   ~/.bash_profile, and the ~/.bashrc it sources returns early in a non-interactive shell (Phase
///   13 A1.6). A new ~/.bash_profile would stop bash from reading ~/.profile, so Devid never makes one.
/// - zsh: ~/.zshenv and ~/.zshrc, as on macOS.
pub fn rc_files_for(shell: &str, home: &Path) -> Vec<PathBuf> {
    if shell.ends_with("zsh") {
        vec![home.join(".zshenv"), home.join(".zshrc")]
    } else if home.join(".bash_profile").is_file() {
        vec![home.join(".bashrc"), home.join(".bash_profile")]
    } else {
        vec![home.join(".bashrc"), home.join(".profile")]
    }
}

pub fn service_path() -> PathBuf {
    user_home().join(".config/systemd/user").join(UNIT)
}

/// A value for a unit file: quoted, with `\`, `"` and systemd's `%` specifiers escaped.
fn unit_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('%', "%%"))
}

pub fn service_unit(paths: &Paths) -> String {
    let bin = install::installed_bin(paths).display().to_string();
    let log = paths.log_dir().join("daemon.log").display().to_string().replace('%', "%%");
    format!(
        "[Unit]\n\
         Description=Devid daemon (local sites, PHP and the web server)\n\
         \n\
         [Service]\n\
         ExecStart={} daemon run\n\
         Environment={}\n\
         # Restart after a crash, but not after `devid daemon stop` (a clean exit).\n\
         Restart=on-failure\n\
         # Only the daemon gets SIGTERM; it stops PHP, the web server and dev servers itself, as under\n\
         # launchd. Whatever is left after it exits is killed.\n\
         KillMode=mixed\n\
         StandardOutput=append:{log}\n\
         StandardError=append:{log}\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n",
        unit_quote(&bin),
        unit_quote(&format!("DEVID_HOME={}", paths.home.display())),
    )
}

fn systemctl_user(args: &[&str]) -> io::Result<()> {
    let out = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .map_err(|e| io::Error::other(format!("systemctl: {e}")))?;
    if out.status.success() {
        return Ok(());
    }
    Err(io::Error::other(format!(
        "`systemctl --user {}` failed: {}; Devid's daemon is a systemd user service, which runs while you \
         are logged in (a desktop or SSH session)",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr).trim()
    )))
}

/// Write the user unit and (re)start it, now and at every login. Lingering stays off: as on
/// macOS, Devid runs while its user is logged in.
pub fn load_service(paths: &Paths) -> io::Result<()> {
    let unit = service_path();
    fs::create_dir_all(unit.parent().expect("unit has a parent"))?;
    fs::create_dir_all(paths.log_dir())?;
    fs::write(&unit, service_unit(paths))?;
    systemctl_user(&["daemon-reload"])?;
    systemctl_user(&["enable", UNIT])?;
    systemctl_user(&["restart", UNIT])
}

/// Stop the daemon; `systemctl stop` returns once it has exited.
pub fn stop_service() {
    let _ = systemctl_user(&["stop", UNIT]);
}

/// Stop and disable the unit and delete it, so nothing starts the daemon at the next login.
pub fn remove_service() {
    let _ = systemctl_user(&["disable", "--now", UNIT]);
    let _ = fs::remove_file(service_path());
    let _ = systemctl_user(&["daemon-reload"]);
}

pub fn service_loaded() -> bool {
    Command::new("systemctl").args(["--user", "is-active", "--quiet", UNIT]).status().is_ok_and(|s| s.success())
}

fn nssdb() -> PathBuf {
    user_home().join(".pki/nssdb")
}

/// Where Firefox keeps its profiles, in Mozilla's older layout and in the XDG one that newer Firefox
/// uses for new profiles (Firefox 160 on Arch made `~/.config/mozilla/firefox`, Phase 13 A4): in the
/// home folder, in Ubuntu's snap, and in Flathub's Flatpak, which Discover installs on Arch's KDE.
const FIREFOX_PROFILES: [&str; 6] = [
    ".mozilla/firefox",
    ".config/mozilla/firefox",
    "snap/firefox/common/.mozilla/firefox",
    "snap/firefox/common/.config/mozilla/firefox",
    ".var/app/org.mozilla.firefox/.mozilla/firefox",
    ".var/app/org.mozilla.firefox/config/mozilla/firefox",
];

/// Every Firefox profile under `home` that has a certificate database. Each profile keeps its own,
/// and Ubuntu's Firefox snap reads nothing else: on the Ubuntu 24.04 machine it rejected a site that
/// curl and Brave trusted until the CA was in its profile (0.19.0-rc.1).
fn firefox_profiles(home: &Path) -> Vec<PathBuf> {
    let mut profiles: Vec<PathBuf> = FIREFOX_PROFILES
        .iter()
        .filter_map(|base| fs::read_dir(home.join(base)).ok())
        .flat_map(|dir| dir.flatten().map(|e| e.path()))
        .filter(|p| p.join("cert9.db").is_file())
        .collect();
    profiles.sort();
    profiles
}

/// certutil: Debian's libnss3-tools (not installed by default), Arch's nss (which Firefox and
/// Chromium depend on).
fn certutil() -> Option<PathBuf> {
    in_path("certutil")
}

fn certutil_run(certutil: &Path, db: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new(certutil)
        .arg("-d")
        .arg(format!("sql:{}", db.display()))
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("certutil: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!("certutil {} ({}): {}", args.join(" "), db.display(), String::from_utf8_lossy(&out.stderr).trim()))
    }
}

/// Remove every copy of the Devid CA from a database; certutil deletes one per call.
fn nss_remove(certutil: &Path, db: &Path) {
    for _ in 0..10 {
        if certutil_run(certutil, db, &["-D", "-n", NSS_NICKNAME]).is_err() {
            break;
        }
    }
}

fn profiles_note(n: usize) -> String {
    if n == 1 { "1 Firefox profile".to_string() } else { format!("{n} Firefox profiles") }
}

/// "~/.pki/nssdb and 2 Firefox profiles", for setup and uninstall.
fn databases_note(firefox: usize) -> String {
    if firefox == 0 { "~/.pki/nssdb".to_string() } else { format!("~/.pki/nssdb and {}", profiles_note(firefox)) }
}

/// Whether setup has a browser step: Chrome and Firefox need one on Linux.
pub fn browser_trust_needed() -> bool {
    true
}

/// The browsers ignore the system store on Ubuntu, and the CA goes into their own NSS databases, as
/// the user, the way mkcert does it (owner's choice, 2026-11-23). Arch's own Firefox and Chromium
/// read the system store through p11-kit, but browsers from elsewhere (Chrome and Brave from the
/// AUR) do not, so Arch gets this step too (D-059, A1):
/// - Chrome and the other Chromium browsers (Brave, Edge) share `~/.pki/nssdb` (L1), which setup
///   creates if it is missing;
/// - every Firefox profile that exists has its own (see `firefox_profiles`). A profile Firefox
///   creates later gets the CA when setup runs again.
///
/// Returns where it went.
pub fn browser_trust(ca: &Path) -> Option<Result<String, String>> {
    Some((|| {
        let certutil = certutil().ok_or_else(|| no_certutil(pacman()))?;
        let chrome = nssdb();
        if !chrome.join("cert9.db").exists() {
            fs::create_dir_all(&chrome).map_err(|e| format!("{}: {e}", chrome.display()))?;
            certutil_run(&certutil, &chrome, &["-N", "--empty-password"])?;
        }
        let firefox = firefox_profiles(&user_home());
        let ca = ca.display().to_string();
        for db in std::iter::once(&chrome).chain(&firefox) {
            nss_remove(&certutil, db);
            certutil_run(&certutil, db, &["-A", "-t", "C,,", "-n", NSS_NICKNAME, "-i", &ca])?;
        }
        Ok(databases_note(firefox.len()))
    })())
}

pub fn browser_untrust() -> Option<Result<String, String>> {
    let certutil = certutil()?;
    let firefox = firefox_profiles(&user_home());
    let chrome = Some(nssdb()).filter(|d| d.join("cert9.db").exists());
    if chrome.is_none() && firefox.is_empty() {
        return None;
    }
    for db in chrome.iter().chain(&firefox) {
        nss_remove(&certutil, db);
    }
    Some(Ok(databases_note(firefox.len())))
}

/// Whether a database holds this CA (not an older one).
fn nss_has(certutil: &Path, db: &Path, ca: &str) -> bool {
    certutil_run(certutil, db, &["-L", "-n", NSS_NICKNAME, "-a"])
        .is_ok_and(|pem| pem_body(&pem).contains(&pem_body(ca)))
}

/// DejaVu Sans where Debian and Arch keep it (devid-binaries' type-linux.xml lists both layouts).
const DEJAVU: [&str; 2] = ["/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", "/usr/share/fonts/TTF/DejaVuSans.ttf"];

/// Checks only Linux needs: the browsers' trust (Chrome's database, and each Firefox profile), and
/// imagick's fonts (devid-binaries' type-linux.xml).
pub fn os_checks(paths: &Paths) -> Vec<Check> {
    let ca = fs::read_to_string(ca::ca_cert(paths)).unwrap_or_default();
    let firefox = firefox_profiles(&user_home());
    let pacman = pacman();
    let mut checks = Vec::new();
    match certutil() {
        None => checks.push((
            false,
            "Chrome and Firefox trust the Devid CA".to_string(),
            format!("install certutil ({}), then run `devid setup`", install_hint(pacman, "libnss3-tools", "nss")),
        )),
        Some(c) => {
            checks.push((
                nss_has(&c, &nssdb(), &ca),
                "Chrome trusts the Devid CA (~/.pki/nssdb)".to_string(),
                "run `devid setup`".to_string(),
            ));
            if !firefox.is_empty() {
                let missing = firefox.iter().filter(|db| !nss_has(&c, db, &ca)).count();
                checks.push((
                    missing == 0,
                    format!("Firefox trusts the Devid CA ({})", profiles_note(firefox.len())),
                    format!("run `devid setup`: {} lack it (Firefox made them after setup)", profiles_note(missing)),
                ));
            }
        }
    }
    let imagick = php::installed(paths).iter().any(|v| phpext::installed(paths, v).iter().any(|e| e.name == "imagick"));
    if imagick {
        checks.push((
            DEJAVU.iter().any(|f| Path::new(f).is_file()),
            "imagick has its fonts (DejaVu)".into(),
            install_hint(pacman, "fonts-dejavu-core fonts-liberation", "ttf-dejavu ttf-liberation"),
        ));
    }
    checks
}

/// Terminals and how each is told its folder, after Debian's x-terminal-emulator alternative and
/// $TERMINAL, which start in the current folder: KDE's and GNOME's first (Arch has no alternative).
const TERMINALS: [(&str, &str); 8] = [
    ("konsole", "--workdir"),
    ("gnome-terminal", "--working-directory"),
    ("kgx", "--working-directory"),
    ("xfce4-terminal", "--working-directory"),
    ("alacritty", "--working-directory"),
    ("kitty", "--directory"),
    ("foot", "--working-directory"),
    ("xterm", ""),
];

fn in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(name)).find(|c| c.is_file())
}

/// The terminal to open and its arguments, for `dir`; `found` says which programs exist. A Flatpak
/// terminal is not on the list: Konsole's (`org.kde.konsole`, which Discover installs on Arch's KDE)
/// starts its shell in the home folder whatever folder it is given (Phase 13 A4).
fn terminal_for(
    dir: &str,
    terminal_env: Option<String>,
    found: impl Fn(&str) -> bool,
) -> Option<(String, Vec<String>)> {
    let mut defaults = std::iter::once("x-terminal-emulator".to_string()).chain(terminal_env.filter(|t| !t.is_empty()));
    if let Some(t) = defaults.find(|t| found(t)) {
        return Some((t, vec![]));
    }
    TERMINALS.iter().find(|(t, _)| found(t)).map(|(t, flag)| match *flag {
        "" => (t.to_string(), vec![]),
        flag => (t.to_string(), vec![flag.to_string(), dir.to_string()]),
    })
}

fn no_terminal(pacman: bool) -> String {
    format!(
        "no terminal that opens in a folder was found: install one ({}) or set $TERMINAL; a Flatpak \
         terminal starts in the home folder",
        install_hint(pacman, "gnome-terminal", "konsole")
    )
}

/// Opens the desktop's terminal in `dir`.
pub fn terminal_command(dir: &str) -> Result<Command, String> {
    let (program, args) = terminal_for(dir, std::env::var("TERMINAL").ok(), |t| in_path(t).is_some())
        .ok_or_else(|| no_terminal(pacman()))?;
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(dir);
    Ok(cmd)
}

/// Apache on macOS is the system httpd, and Linux has none to borrow (D-057).
pub fn unavailable_server(name: &str) -> Option<String> {
    (name == "apache").then(|| "Apache is not available on Linux yet; use nginx or Caddy".to_string())
}

/// The inodes of established TCP sockets with local port `port`, from a /proc/net/tcp{,6} table.
fn established_inodes(table: &str, port: u16) -> impl Iterator<Item = u64> + '_ {
    table.lines().skip(1).filter_map(move |line| {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let (local, state, inode) = (fields.get(1)?, fields.get(3)?, fields.get(9)?);
        let local_port = u16::from_str_radix(local.rsplit(':').next()?, 16).ok()?;
        // 01 is TCP_ESTABLISHED.
        (*state == "01" && local_port == port).then(|| inode.parse().ok())?
    })
}

/// The process group in a /proc/<pid>/stat line. The command name may hold spaces and
/// parentheses; the fields after the last `)` are state, ppid and pgrp.
fn stat_pgrp(stat: &str) -> Option<i32> {
    stat.rsplit_once(')')?.1.split_whitespace().nth(2)?.parse().ok()
}

/// Whether a process of group `pgid` has an established TCP connection on its local port `port`
/// (see the macOS twin). Read from /proc without starting a process: the established sockets on
/// that port, matched by inode against the descriptors of the group's processes (L1's prototype).
pub fn established(pgid: i32, port: u16) -> bool {
    let inodes: HashSet<u64> = ["/proc/net/tcp", "/proc/net/tcp6"]
        .iter()
        .filter_map(|t| fs::read_to_string(t).ok())
        .flat_map(|t| established_inodes(&t, port).collect::<Vec<_>>())
        .collect();
    if inodes.is_empty() {
        return false;
    }
    let Ok(procs) = fs::read_dir("/proc") else { return false };
    procs.flatten().filter(|e| e.file_name().to_str().is_some_and(|n| n.bytes().all(|b| b.is_ascii_digit()))).any(|e| {
        let dir = e.path();
        if fs::read_to_string(dir.join("stat")).ok().and_then(|s| stat_pgrp(&s)) != Some(pgid) {
            return false;
        }
        let Ok(fds) = fs::read_dir(dir.join("fd")) else { return false };
        fds.flatten().any(|fd| {
            fs::read_link(fd.path()).ok().is_some_and(|l| {
                l.to_str()
                    .and_then(|l| l.strip_prefix("socket:[")?.strip_suffix(']')?.parse::<u64>().ok())
                    .is_some_and(|i| inodes.contains(&i))
            })
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> Plan {
        Plan {
            tld: "test".into(),
            dns_port: 53535,
            http_port: 8088,
            https_port: 8443,
            ca_cert: "/home/me/.devid/ca/devid-ca.crt".into(),
        }
    }

    #[test]
    fn generated_files() {
        let p = plan();
        let dropin = resolved_dropin(&p);
        assert!(dropin.contains("[Resolve]\nDNS=127.0.0.1:53535\nDomains=~test\n"), "only .test goes to Devid");
        let rules = nft_rules(&p);
        assert!(rules.contains("table inet devid\ndelete table inet devid\n"), "reloading replaces the table");
        assert!(rules.contains("type nat hook output priority -100;"), "local traffic only");
        assert!(rules.contains("ip daddr 127.0.0.1 tcp dport 80 redirect to :8088"));
        assert!(rules.contains("ip daddr 127.0.0.1 tcp dport 443 redirect to :8443"));
        let unit = ports_unit();
        assert!(unit.contains("ExecStart=/usr/sbin/nft -f /etc/devid-ports.nft"));
        assert!(unit.contains("ExecStop=/usr/sbin/nft delete table inet devid"));
        assert!(unit.contains("After=network-pre.target nftables.service"));
        assert!(unit.contains("RemainAfterExit=yes") && unit.contains("WantedBy=multi-user.target"));
    }

    /// Ubuntu: systemd-resolved runs and Debian's CA tool is there (0.19's steps, unchanged).
    fn ubuntu() -> System {
        System {
            p11kit: false,
            resolved_active: true,
            resolved_ours: false,
            resolved_sockets: vec![],
            resolv_backup: false,
            resolv_stub: true,
        }
    }

    /// A stock Arch desktop before setup (A1): NetworkManager writes /etc/resolv.conf, resolved is off.
    fn arch() -> System {
        System { p11kit: true, resolved_active: false, resolv_stub: false, ..ubuntu() }
    }

    /// The same Arch machine after setup turned resolved on.
    fn arch_after_setup() -> System {
        System {
            resolved_active: true,
            resolved_ours: true,
            resolved_sockets: vec!["systemd-resolved-varlink.socket".into(), "systemd-resolved-monitor.socket".into()],
            resolv_backup: true,
            resolv_stub: true,
            ..arch()
        }
    }

    fn described(actions: &[Action]) -> Vec<String> {
        actions.iter().map(Action::describe).collect()
    }

    #[test]
    fn install_and_uninstall() {
        let p = plan();
        let install = install_actions_on(&p, &ubuntu());
        assert!(install.contains(&Action::Copy { from: p.ca_cert.clone(), to: CA_FILE.into() }));
        let runs: Vec<String> =
            install.iter().filter_map(|a| if let Action::Run { .. } = a { Some(a.describe()) } else { None }).collect();
        assert_eq!(
            runs,
            [
                "run /usr/bin/systemctl daemon-reload",
                "run /usr/bin/systemctl restart systemd-resolved",
                "run /usr/bin/systemctl enable devid-ports.service",
                "run /usr/bin/systemctl restart devid-ports.service",
                "run /usr/sbin/update-ca-certificates",
            ]
        );
        assert!(!described(&install).iter().any(|a| a.contains("resolv.conf") || a.contains(RESOLVED_STATE)));
        // The CA copy comes before update-ca-certificates, the unit after the rules it loads.
        let at = |want: &str| install.iter().position(|a| a.describe().starts_with(want)).unwrap();
        assert!(at("copy") < at("run /usr/sbin/update-ca-certificates"));
        assert!(at("write /etc/devid-ports.nft") < at("run /usr/bin/systemctl restart devid-ports"));
        let uninstall = uninstall_actions_on(&p, &ubuntu());
        super::super::tests::uninstall_removes_every_file(&install, &uninstall);
        assert!(uninstall.contains(&Action::Remove { path: RESOLVED_DROPIN.into(), restore_backup: true }));
        assert!(uninstall.iter().any(|a| a.describe() == "run /usr/sbin/update-ca-certificates --fresh"));
        assert!(uninstall.iter().any(|a| a.describe() == "run /usr/bin/systemctl disable --now devid-ports.service"));
        assert!(uninstall.iter().any(|a| a.describe() == "run /usr/bin/systemctl restart systemd-resolved"));
    }

    /// Arch (D-059, A1): the CA through p11-kit's anchors, and systemd-resolved turned on with
    /// /etc/resolv.conf linked to its stub; uninstall puts back the file and turns resolved and its
    /// sockets off.
    #[test]
    fn arch_turns_resolved_on_and_uninstall_turns_it_off() {
        let p = plan();
        let install = install_actions_on(&p, &arch());
        let steps = described(&install);
        assert_eq!(steps[0], format!("write {RESOLVED_STATE}"), "noted first, for an uninstall after a failure");
        let at = |want: &str| steps.iter().position(|s| s == want).unwrap_or_else(|| panic!("{want}: {steps:#?}"));
        let enable = at("run /usr/bin/systemctl enable systemd-resolved.service");
        let restart = at("run /usr/bin/systemctl restart systemd-resolved");
        let link = at("link /etc/resolv.conf to ../run/systemd/resolve/stub-resolv.conf (backing up what is there)");
        let network = at("run /usr/bin/systemctl try-restart NetworkManager.service");
        assert!(at("write /etc/systemd/resolved.conf.d/devid.conf (backing up an existing file)") < restart);
        assert!(enable < restart && restart < link && link < network, "{steps:#?}");
        assert!(install.contains(&Action::Copy { from: p.ca_cert.clone(), to: CA_ANCHOR.into() }));
        assert!(
            at(&format!("copy {} to {CA_ANCHOR}", p.ca_cert.display())) < at("run /usr/bin/update-ca-trust extract")
        );
        assert!(!steps.iter().any(|s| s.contains("update-ca-certificates")));

        // A second setup, with resolved already on, keeps 0.19's steps: no second link or note.
        let again = described(&install_actions_on(&p, &arch_after_setup()));
        assert!(!again.iter().any(|s| s.contains("resolv.conf") || s.contains(RESOLVED_STATE)), "{again:#?}");

        let uninstall = uninstall_actions_on(&p, &arch_after_setup());
        super::super::tests::uninstall_removes_every_file(&install, &uninstall);
        let undo = described(&uninstall);
        let at = |want: &str| undo.iter().position(|s| s == want).unwrap_or_else(|| panic!("{want}: {undo:#?}"));
        let back = at("put back /etc/resolv.conf from /etc/resolv.conf.devid-backup");
        let off = at("run /usr/bin/systemctl disable --now systemd-resolved.service systemd-resolved-varlink.socket \
             systemd-resolved-monitor.socket");
        let network = at("run /usr/bin/systemctl try-restart NetworkManager.service");
        let note = at(&format!("remove {RESOLVED_STATE}"));
        assert!(back < off && off < network && network < note, "{undo:#?}");
        assert!(!undo.iter().any(|s| s == "run /usr/bin/systemctl restart systemd-resolved"));
        assert!(uninstall.contains(&Action::Remove { path: CA_ANCHOR.into(), restore_backup: false }));
        assert!(undo.iter().any(|s| s == "run /usr/bin/update-ca-trust extract"));

        // Devid's link with nothing saved behind it goes, and NetworkManager writes a new file;
        // with neither, /etc/resolv.conf is left alone. Never `remove` with a restore that finds none.
        let no_backup = System { resolv_backup: false, ..arch_after_setup() };
        assert!(
            uninstall_actions_on(&p, &no_backup)
                .contains(&Action::Remove { path: RESOLV_CONF.into(), restore_backup: false })
        );
        let foreign = System { resolv_backup: false, resolv_stub: false, ..arch_after_setup() };
        assert!(!described(&uninstall_actions_on(&p, &foreign)).iter().any(|s| s.contains("/etc/resolv.conf")));
    }

    #[test]
    fn prerequisites_name_the_packages_for_apt_or_pacman() {
        let ubuntu = |p: &str| [SYSTEMCTL, NFT, UPDATE_CA].contains(&p);
        assert_eq!(prerequisites_on(ubuntu, true, false), Ok(()));
        let arch = |p: &str| [SYSTEMCTL, NFT, UPDATE_CA_TRUST, TRUST_SOURCE, PACMAN].contains(&p);
        assert_eq!(prerequisites_on(arch, false, true), Ok(()), "setup turns resolved on next to NetworkManager");
        let no_nft = |p: &str| p != NFT && arch(p);
        assert_eq!(
            prerequisites_on(no_nft, true, true).unwrap_err(),
            "the system setup needs nftables: `sudo pacman -S --needed nftables`"
        );
        let bare = |p: &str| p == SYSTEMCTL;
        assert_eq!(
            prerequisites_on(bare, true, true).unwrap_err(),
            "the system setup needs nftables ca-certificates: `sudo apt install nftables ca-certificates`"
        );
        let err = prerequisites_on(arch, false, false).unwrap_err();
        assert!(err.contains("does not run here") && err.contains("NetworkManager"), "{err}");
        assert!(no_certutil(true).contains("`sudo pacman -S --needed nss`"));
        assert!(no_certutil(false).contains("`sudo apt install libnss3-tools`"));
    }

    #[test]
    fn the_terminal_is_found_and_told_its_folder() {
        let only = |names: &'static [&'static str]| move |t: &str| names.contains(&t);
        assert_eq!(
            terminal_for("/p", None, only(&["x-terminal-emulator", "konsole"])),
            Some(("x-terminal-emulator".into(), vec![]))
        );
        assert_eq!(terminal_for("/p", Some("foot".into()), only(&["foot", "konsole"])), Some(("foot".into(), vec![])));
        assert_eq!(
            terminal_for("/p", None, only(&["konsole", "gnome-terminal"])),
            Some(("konsole".into(), vec!["--workdir".into(), "/p".into()]))
        );
        assert_eq!(
            terminal_for("/p", Some(String::new()), only(&["kgx"])),
            Some(("kgx".into(), vec!["--working-directory".into(), "/p".into()]))
        );
        assert_eq!(terminal_for("/p", None, only(&["xterm"])), Some(("xterm".into(), vec![])));
        // Arch's KDE with only Discover's Flatpak Konsole: no terminal, and a message that says why.
        assert_eq!(terminal_for("/p", None, only(&["org.kde.konsole"])), None);
        assert!(no_terminal(true).contains("`sudo pacman -S --needed konsole`"));
        assert!(no_terminal(false).contains("`sudo apt install gnome-terminal`"));
    }

    #[test]
    fn the_bundle_check_ignores_line_breaks() {
        let ca = "-----BEGIN CERTIFICATE-----\nAAAA\nBBBB\n-----END CERTIFICATE-----\n";
        let bundle = "-----BEGIN CERTIFICATE-----\nZZZZ\n-----END CERTIFICATE-----\n\
                      -----BEGIN CERTIFICATE-----\r\nAAAA\r\nBBBB\r\n-----END CERTIFICATE-----\r\n";
        assert!(bundle_has(bundle, ca));
        assert!(!bundle_has("-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----\n", ca));
        assert!(!bundle_has(bundle, ""), "no CA is never in the bundle");
    }

    #[test]
    fn bash_gets_bashrc_and_profile_zsh_gets_zshenv_and_zshrc() {
        let home = std::env::temp_dir().join(format!("devid-rc-linux-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(&home).unwrap();
        assert_eq!(rc_files_for("/bin/bash", &home), [home.join(".bashrc"), home.join(".profile")]);
        assert_eq!(rc_files_for("", &home), [home.join(".bashrc"), home.join(".profile")], "bash is the default");
        assert_eq!(rc_files_for("/usr/bin/zsh", &home), [home.join(".zshenv"), home.join(".zshrc")]);
        // Arch's /etc/skel: bash's login shells read ~/.bash_profile and never ~/.profile (A1.6).
        fs::write(home.join(".bash_profile"), "[[ -f ~/.bashrc ]] && . ~/.bashrc\n").unwrap();
        assert_eq!(rc_files_for("/usr/bin/bash", &home), [home.join(".bashrc"), home.join(".bash_profile")]);
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn service_unit_runs_the_installed_binary() {
        let paths = Paths { home: PathBuf::from("/home/me/.devid") };
        let unit = service_unit(&paths);
        assert!(unit.contains("ExecStart=\"/home/me/.devid/bin/devid\" daemon run\n"));
        assert!(unit.contains("Environment=\"DEVID_HOME=/home/me/.devid\"\n"));
        assert!(unit.contains("Restart=on-failure\n"), "no restart after a clean stop");
        assert!(unit.contains("KillMode=mixed\n"), "the daemon stops its children itself");
        assert!(unit.contains("StandardOutput=append:/home/me/.devid/log/daemon.log\n"));
        assert!(unit.contains("WantedBy=default.target\n"));
        let odd = service_unit(&Paths { home: PathBuf::from("/home/a \"b\" 100%/.devid") });
        assert!(odd.contains(r#"ExecStart="/home/a \"b\" 100%%/.devid/bin/devid" daemon run"#), "{odd}");
    }

    #[test]
    fn parses_proc_tables() {
        let table = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n\
            0: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 111 1 0000\n\
            1: 0100007F:1F90 0100007F:D431 01 00000000:00000000 00:00000000 00000000  1000        0 222 1 0000\n\
            2: 0100007F:D431 0100007F:1F90 01 00000000:00000000 00:00000000 00000000  1000        0 333 1 0000\n";
        assert_eq!(established_inodes(table, 0x1F90).collect::<Vec<_>>(), [222], "established, local port 8080");
        assert_eq!(established_inodes(table, 80).count(), 0);
        assert_eq!(stat_pgrp("4242 (node) S 1 4240 4240 0 -1"), Some(4240));
        assert_eq!(stat_pgrp("4242 (a) b (c)) S 1 77 77 0 -1"), Some(77), "a name with parentheses");
        assert_eq!(stat_pgrp("garbage"), None);
    }

    #[test]
    fn finds_every_firefox_profile_with_a_database() {
        let home = std::env::temp_dir().join(format!("devid-ff-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        for (dir, db) in [
            (".mozilla/firefox/abc.default-release", true),
            (".mozilla/firefox/Crash Reports", false),
            // Firefox 160 on Arch (Phase 13 A4).
            (".config/mozilla/firefox/m1bhyes5.default-release", true),
            ("snap/firefox/common/.mozilla/firefox/xyz.default", true),
            ("snap/firefox/common/.mozilla/firefox/Profile Groups", false),
            (".var/app/org.mozilla.firefox/config/mozilla/firefox/fp.default-release", true),
        ] {
            fs::create_dir_all(home.join(dir)).unwrap();
            if db {
                fs::write(home.join(dir).join("cert9.db"), "").unwrap();
            }
        }
        assert_eq!(
            firefox_profiles(&home),
            [
                home.join(".config/mozilla/firefox/m1bhyes5.default-release"),
                home.join(".mozilla/firefox/abc.default-release"),
                home.join(".var/app/org.mozilla.firefox/config/mozilla/firefox/fp.default-release"),
                home.join("snap/firefox/common/.mozilla/firefox/xyz.default"),
            ],
            "the profiles in each layout, not their other folders"
        );
        assert!(firefox_profiles(&home.join("nobody")).is_empty());
        assert_eq!(databases_note(0), "~/.pki/nssdb");
        assert_eq!(databases_note(1), "~/.pki/nssdb and 1 Firefox profile");
        assert_eq!(databases_note(2), "~/.pki/nssdb and 2 Firefox profiles");
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn apache_is_refused() {
        assert!(unavailable_server("apache").is_some());
        assert!(unavailable_server("nginx").is_none() && unavailable_server("caddy").is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn an_open_connection_is_seen() {
        super::super::tests::sees_an_open_connection(established);
    }
}
