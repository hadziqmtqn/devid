//! Linux, Ubuntu first (D-057, Phase 12 L1): systemd-resolved for `.<tld>`, an nftables table
//! loaded by a system unit for ports 80/443, the system CA store, `certutil` for Chrome's own
//! database, and a systemd user unit for the daemon.

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
pub const NFT_FILE: &str = "/etc/devid-ports.nft";
pub const PORTS_UNIT: &str = "/etc/systemd/system/devid-ports.service";
pub const CA_FILE: &str = "/usr/local/share/ca-certificates/devid-ca.crt";
pub const CA_BUNDLE: &str = "/etc/ssl/certs/ca-certificates.crt";
const SYSTEMCTL: &str = "/usr/bin/systemctl";
const NFT: &str = "/usr/sbin/nft";
const UPDATE_CA: &str = "/usr/sbin/update-ca-certificates";
const UNIT: &str = "devid.service";
const NSS_NICKNAME: &str = "Devid Local CA";
const NO_CERTUTIL: &str = "Chrome does not trust https://*.test yet: install certutil (`sudo apt install \
     libnss3-tools`), then run `devid setup` again";

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
    vec![
        Action::Write { path: RESOLVED_DROPIN.into(), content: resolved_dropin(plan), backup: true },
        run(SYSTEMCTL, &["restart", "systemd-resolved"], false),
        Action::Write { path: NFT_FILE.into(), content: nft_rules(plan), backup: false },
        Action::Write { path: PORTS_UNIT.into(), content: ports_unit(), backup: false },
        run(SYSTEMCTL, &["daemon-reload"], false),
        run(SYSTEMCTL, &["enable", "devid-ports.service"], false),
        // restart, not start: a running unit must load changed ports.
        run(SYSTEMCTL, &["restart", "devid-ports.service"], false),
        Action::Copy { from: plan.ca_cert.clone(), to: CA_FILE.into() },
        run(UPDATE_CA, &[], false),
    ]
}

pub fn uninstall_actions(_plan: &Plan) -> Vec<Action> {
    vec![
        // Stopping runs the unit's ExecStop, which deletes the table.
        run(SYSTEMCTL, &["disable", "--now", "devid-ports.service"], true),
        Action::Remove { path: PORTS_UNIT.into(), restore_backup: false },
        run(SYSTEMCTL, &["daemon-reload"], true),
        Action::Remove { path: NFT_FILE.into(), restore_backup: false },
        Action::Remove { path: RESOLVED_DROPIN.into(), restore_backup: true },
        run(SYSTEMCTL, &["restart", "systemd-resolved"], true),
        Action::Remove { path: CA_FILE.into(), restore_backup: false },
        run(UPDATE_CA, &["--fresh"], true),
    ]
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
    let resolver = fs::read_to_string(RESOLVED_DROPIN).is_ok_and(|c| c == resolved_dropin(plan));
    let ports = redirect_installed(plan) && fs::read_to_string(PORTS_UNIT).is_ok_and(|c| c == ports_unit());
    let trusted = match (fs::read_to_string(&plan.ca_cert), fs::read_to_string(CA_FILE)) {
        (Ok(ca), Ok(copy)) => ca == copy && fs::read_to_string(CA_BUNDLE).is_ok_and(|b| bundle_has(&b, &ca)),
        _ => false,
    };
    vec![("resolver", resolver), ("ports 80/443", ports), ("CA trusted", trusted)]
}

/// Whether the redirect for these ports is installed, so URLs need no port.
pub fn redirect_installed(plan: &Plan) -> bool {
    fs::read_to_string(NFT_FILE).is_ok_and(|c| c == nft_rules(plan))
}

/// What the root step needs, checked before `sudo`: the tools, and systemd-resolved running.
pub fn prerequisites() -> Result<(), String> {
    let missing: Vec<&str> = [(SYSTEMCTL, "systemd"), (NFT, "nftables"), (UPDATE_CA, "ca-certificates")]
        .into_iter()
        .filter(|(path, _)| !Path::new(path).exists())
        .map(|(_, package)| package)
        .collect();
    if !missing.is_empty() {
        return Err(format!("the system setup needs {0}: `sudo apt install {0}`", missing.join(" ")));
    }
    let resolved =
        Command::new(SYSTEMCTL).args(["is-active", "--quiet", "systemd-resolved"]).status().is_ok_and(|s| s.success());
    if !resolved {
        return Err("systemd-resolved does not run, and Devid sends .test lookups through it (D-057); \
             Ubuntu runs it by default"
            .into());
    }
    Ok(())
}

/// The startup files that put `~/.devid/bin` on PATH (D-053), for the login shell:
/// - bash (the default): ~/.bashrc, which the terminal's non-login shells read; and ~/.profile,
///   which the desktop session reads at login, so editors started from the menu find `php`. A new
///   ~/.bash_profile would stop bash from reading ~/.profile.
/// - zsh: ~/.zshenv and ~/.zshrc, as on macOS.
pub fn rc_files_for(shell: &str, home: &Path) -> Vec<PathBuf> {
    if shell.ends_with("zsh") {
        vec![home.join(".zshenv"), home.join(".zshrc")]
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

/// certutil, from libnss3-tools; not installed by default.
fn certutil() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join("certutil")).find(|c| c.is_file())
}

fn certutil_run(certutil: &Path, args: &[&str]) -> Result<String, String> {
    let db = format!("sql:{}", nssdb().display());
    let out = Command::new(certutil)
        .args(["-d", &db])
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("certutil: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!("certutil {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()))
    }
}

/// Remove every copy of the Devid CA from the database; certutil deletes one per call.
fn nss_remove(certutil: &Path) {
    for _ in 0..10 {
        if certutil_run(certutil, &["-D", "-n", NSS_NICKNAME]).is_err() {
            break;
        }
    }
}

/// Whether setup has a browser step: Chrome needs one on Linux.
pub fn browser_trust_needed() -> bool {
    true
}

/// Chrome and the other Chromium browsers ignore the system store on Linux (L1) and read the
/// user's NSS database, `~/.pki/nssdb`. The CA goes there, as the user, the way mkcert does it
/// (owner's choice, 2026-11-23). Returns the database it went into.
pub fn browser_trust(ca: &Path) -> Option<Result<String, String>> {
    Some((|| {
        let certutil = certutil().ok_or(NO_CERTUTIL)?;
        let dir = nssdb();
        if !dir.join("cert9.db").exists() {
            fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            certutil_run(&certutil, &["-N", "--empty-password"])?;
        }
        nss_remove(&certutil);
        let ca = ca.display().to_string();
        certutil_run(&certutil, &["-A", "-t", "C,,", "-n", NSS_NICKNAME, "-i", &ca])?;
        Ok(dir.display().to_string())
    })())
}

pub fn browser_untrust() -> Option<Result<String, String>> {
    let certutil = certutil()?;
    if !nssdb().join("cert9.db").exists() {
        return None;
    }
    nss_remove(&certutil);
    Some(Ok(nssdb().display().to_string()))
}

/// Whether Chrome's database holds this CA (not an older one).
fn nss_has(certutil: &Path, ca: &Path) -> bool {
    let Ok(ca) = fs::read_to_string(ca) else { return false };
    certutil_run(certutil, &["-L", "-n", NSS_NICKNAME, "-a"]).is_ok_and(|pem| pem_body(&pem).contains(&pem_body(&ca)))
}

const DEJAVU: &str = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";

/// Checks only Linux needs: Chrome's trust, and imagick's fonts (devid-binaries' type-linux.xml).
pub fn os_checks(paths: &Paths) -> Vec<Check> {
    let ca = ca::ca_cert(paths);
    let (trusted, fix) = match certutil() {
        None => (false, "install libnss3-tools (`sudo apt install libnss3-tools`), then run `devid setup`"),
        Some(c) => (nss_has(&c, &ca), "run `devid setup`"),
    };
    let mut checks = vec![(trusted, "Chrome trusts the Devid CA (~/.pki/nssdb)".to_string(), fix.to_string())];
    let imagick = php::installed(paths).iter().any(|v| phpext::installed(paths, v).iter().any(|e| e.name == "imagick"));
    if imagick {
        checks.push((
            Path::new(DEJAVU).is_file(),
            "imagick has its fonts (DejaVu)".into(),
            "`sudo apt install fonts-dejavu-core fonts-liberation`".into(),
        ));
    }
    checks
}

/// Opens the desktop's terminal in `dir` (Debian's x-terminal-emulator alternative).
pub fn terminal_command(dir: &str) -> Command {
    let mut cmd = Command::new("x-terminal-emulator");
    cmd.current_dir(dir);
    cmd
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

    #[test]
    fn install_and_uninstall() {
        let p = plan();
        let install = install_actions(&p);
        assert!(install.contains(&Action::Copy { from: p.ca_cert.clone(), to: CA_FILE.into() }));
        let runs: Vec<String> =
            install.iter().filter_map(|a| if let Action::Run { .. } = a { Some(a.describe()) } else { None }).collect();
        assert_eq!(
            runs,
            [
                "run /usr/bin/systemctl restart systemd-resolved",
                "run /usr/bin/systemctl daemon-reload",
                "run /usr/bin/systemctl enable devid-ports.service",
                "run /usr/bin/systemctl restart devid-ports.service",
                "run /usr/sbin/update-ca-certificates",
            ]
        );
        // The CA copy comes before update-ca-certificates, the unit after the rules it loads.
        let at = |want: &str| install.iter().position(|a| a.describe().starts_with(want)).unwrap();
        assert!(at("copy") < at("run /usr/sbin/update-ca-certificates"));
        assert!(at("write /etc/devid-ports.nft") < at("run /usr/bin/systemctl restart devid-ports"));
        let uninstall = uninstall_actions(&p);
        super::super::tests::uninstall_removes_every_file(&install, &uninstall);
        assert!(uninstall.contains(&Action::Remove { path: RESOLVED_DROPIN.into(), restore_backup: true }));
        assert!(uninstall.iter().any(|a| a.describe() == "run /usr/sbin/update-ca-certificates --fresh"));
        assert!(uninstall.iter().any(|a| a.describe() == "run /usr/bin/systemctl disable --now devid-ports.service"));
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
        let home = Path::new("/home/me");
        assert_eq!(rc_files_for("/bin/bash", home), [home.join(".bashrc"), home.join(".profile")]);
        assert_eq!(rc_files_for("", home), [home.join(".bashrc"), home.join(".profile")], "bash is the default");
        assert_eq!(rc_files_for("/usr/bin/zsh", home), [home.join(".zshenv"), home.join(".zshrc")]);
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
