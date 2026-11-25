//! macOS: /etc/resolver, a pf anchor with a LaunchDaemon, the System keychain, and a LaunchAgent
//! for the daemon (D-002, D-014, D-047).

use super::Check;
use crate::{
    helper::{Action, Plan},
    install,
    paths::{Paths, user_home},
};
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

pub const NAME: &str = "macOS";
pub const OPEN_COMMAND: &str = "open";
pub const COPY_COMMANDS: &[&[&str]] = &[&["pbcopy"]];
/// Why ports 80/443 can stop reaching the web server, and the fix (D-047).
pub const REDIRECT_FIX: &str = "another app switched pf off on loopback (often OrbStack, Docker, UTM or Internet \
     Sharing); run `devid helper install`";
pub const SERVICE: &str = "LaunchAgent dev.devid.daemon";
/// The desktop app (Phase 7) runs here.
pub const DESKTOP_APP: bool = true;

pub const PF_ANCHOR: &str = "com.apple/devid";
pub const PF_FILE: &str = "/etc/pf.anchors/dev.devid";
pub const PF_PLIST: &str = "/Library/LaunchDaemons/dev.devid.pf.plist";
pub const PF_LABEL: &str = "dev.devid.pf";
const SYSTEM_KEYCHAIN: &str = "/Library/Keychains/System.keychain";
const CA_NAME: &str = "Devid Local CA";
pub const AGENT_LABEL: &str = "dev.devid.daemon";

/// This Mac's devid binary in a devid release (self-update, install.sh).
pub const DEVID_ASSET: &str = if cfg!(target_arch = "aarch64") { "devid-darwin-arm64" } else { "devid-darwin-x86_64" };

/// The arch key of this Mac's builds in the runtime manifest.
pub fn arch_key() -> &'static str {
    if cfg!(target_arch = "aarch64") { "arm64" } else { "x86_64" }
}

pub fn resolver_path(tld: &str) -> PathBuf {
    PathBuf::from(format!("/etc/resolver/{tld}"))
}

pub fn resolver_content(plan: &Plan) -> String {
    format!("# Devid: send .{} lookups to the Devid daemon\nnameserver 127.0.0.1\nport {}\n", plan.tld, plan.dns_port)
}

pub fn pf_rules(plan: &Plan) -> String {
    format!(
        "# Devid: local-only redirect of the standard web ports to nginx's high ports\n\
         rdr pass on lo0 inet proto tcp from any to 127.0.0.1 port 80 -> 127.0.0.1 port {}\n\
         rdr pass on lo0 inet proto tcp from any to 127.0.0.1 port 443 -> 127.0.0.1 port {}\n",
        plan.http_port, plan.https_port
    )
}

fn pf_load_command() -> String {
    // -E enables pf (it is off by default on macOS) and takes a reference so others' -X cannot disable it.
    format!("/sbin/pfctl -E >/dev/null 2>&1; /sbin/pfctl -a {PF_ANCHOR} -f {PF_FILE}")
}

/// Another pf client can set `skip on lo0` (macOS Internet Sharing does when OrbStack, Docker or UTM
/// adds a VM network); the `rdr … on lo0` rules then stop matching and 80/443 refuse connections.
/// Reloading Devid's anchor does not undo it, reloading pf's options from /etc/pf.conf does (D-047).
/// Only when it is set, so a working setup is left as it is.
const PF_UNSKIP_LO0: &str = "/sbin/pfctl -s Interfaces -v 2>/dev/null | /usr/bin/grep -q '^lo0 (skip)' \
    && /sbin/pfctl -O -f /etc/pf.conf >/dev/null 2>&1; exit 0";

fn pf_plist() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{PF_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>/bin/sh</string>
        <string>-c</string>
        <string>{cmd}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
"#,
        cmd = pf_load_command()
    )
}

fn run(program: &str, args: &[&str], ok_to_fail: bool) -> Action {
    Action::Run { program: program.into(), args: args.iter().map(|s| s.to_string()).collect(), ok_to_fail }
}

pub fn install_actions(plan: &Plan) -> Vec<Action> {
    let ca = plan.ca_cert.display().to_string();
    vec![
        Action::Write { path: resolver_path(&plan.tld), content: resolver_content(plan), backup: true },
        Action::Write { path: PF_FILE.into(), content: pf_rules(plan), backup: false },
        Action::Write { path: PF_PLIST.into(), content: pf_plist(), backup: false },
        run("/bin/sh", &["-c", &pf_load_command()], false),
        run("/bin/sh", &["-c", PF_UNSKIP_LO0], true),
        // Replace any earlier Devid CA in the System keychain, then trust the current one for SSL.
        run("/usr/bin/security", &["delete-certificate", "-c", CA_NAME, SYSTEM_KEYCHAIN], true),
        run(
            "/usr/bin/security",
            &["add-trusted-cert", "-d", "-r", "trustRoot", "-p", "ssl", "-k", SYSTEM_KEYCHAIN, &ca],
            false,
        ),
        run("/usr/bin/dscacheutil", &["-flushcache"], true),
        run("/usr/bin/killall", &["-HUP", "mDNSResponder"], true),
    ]
}

pub fn uninstall_actions(plan: &Plan) -> Vec<Action> {
    let ca = plan.ca_cert.display().to_string();
    vec![
        run("/sbin/pfctl", &["-a", PF_ANCHOR, "-F", "all"], true),
        Action::Remove { path: PF_PLIST.into(), restore_backup: false },
        Action::Remove { path: PF_FILE.into(), restore_backup: false },
        Action::Remove { path: resolver_path(&plan.tld), restore_backup: true },
        run("/usr/bin/security", &["remove-trusted-cert", "-d", &ca], true),
        run("/usr/bin/security", &["delete-certificate", "-c", CA_NAME, SYSTEM_KEYCHAIN], true),
        run("/usr/bin/dscacheutil", &["-flushcache"], true),
        run("/usr/bin/killall", &["-HUP", "mDNSResponder"], true),
    ]
}

pub fn system_status(plan: &Plan) -> Vec<(&'static str, bool)> {
    let resolver = fs::read_to_string(resolver_path(&plan.tld)).is_ok_and(|c| c == resolver_content(plan));
    let pf = redirect_installed(plan) && Path::new(PF_PLIST).exists();
    let trusted = Command::new("/usr/bin/security")
        .args(["verify-cert", "-q", "-L", "-c"])
        .arg(&plan.ca_cert)
        .args(["-l", "-p", "ssl"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    vec![("resolver", resolver), ("ports 80/443", pf), ("CA trusted", trusted)]
}

/// Whether the redirect for these ports is installed, so URLs need no port.
pub fn redirect_installed(plan: &Plan) -> bool {
    fs::read_to_string(PF_FILE).is_ok_and(|c| c == pf_rules(plan))
}

/// Everything the root step needs comes with macOS.
pub fn prerequisites() -> Result<(), String> {
    Ok(())
}

/// The startup files that put `~/.devid/bin` on PATH (D-053), for the login shell:
/// - zsh (the macOS default): ~/.zshenv, which every zsh reads, also the shells that editors,
///   scripts and other tools start; and ~/.zshrc, because in a terminal macOS's path_helper
///   (/etc/zprofile) runs after ~/.zshenv and moves its entries behind /usr/local/bin. The line in
///   ~/.zshrc, read last, puts Devid's `php` before Homebrew's again.
/// - bash: ~/.bash_profile.
pub fn rc_files_for(shell: &str, home: &Path) -> Vec<PathBuf> {
    if shell.ends_with("bash") {
        vec![home.join(".bash_profile")]
    } else {
        vec![home.join(".zshenv"), home.join(".zshrc")]
    }
}

pub fn service_path() -> PathBuf {
    user_home().join("Library/LaunchAgents").join(format!("{AGENT_LABEL}.plist"))
}

pub fn agent_plist(paths: &Paths) -> String {
    let log = paths.log_dir().join("daemon.log");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{AGENT_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{bin}</string>
        <string>daemon</string>
        <string>run</string>
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>DEVID_HOME</key>
        <string>{home}</string>
    </dict>
    <key>RunAtLoad</key>
    <true/>
    <!-- Restart after a crash, but not after `devid daemon stop` (a clean exit). -->
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <!-- No ProcessType: "Background" ran nginx and php-fpm at the lowest CPU priority, so pages
         took seconds whenever the Mac was busy. -->
    <!-- A database may take up to a minute to stop cleanly (D-061); launchd's default 20 s would
         kill it halfway. -->
    <key>ExitTimeOut</key>
    <integer>{exit_timeout}</integer>
    <key>StandardErrorPath</key>
    <string>{log}</string>
    <key>StandardOutPath</key>
    <string>{log}</string>
</dict>
</plist>
"#,
        bin = install::installed_bin(paths).display(),
        home = paths.home.display(),
        log = log.display(),
        exit_timeout = super::EXIT_TIMEOUT,
    )
}

fn gui_domain() -> String {
    // SAFETY: getuid has no preconditions.
    format!("gui/{}", unsafe { libc::getuid() })
}

/// Write the LaunchAgent and (re)load it so it starts the daemon now and at every login.
pub fn load_service(paths: &Paths) -> io::Result<()> {
    let plist = service_path();
    fs::create_dir_all(plist.parent().expect("plist has a parent"))?;
    fs::create_dir_all(paths.log_dir())?;
    fs::write(&plist, agent_plist(paths))?;
    stop_service();
    // Even after the old job is gone, launchd can briefly refuse the new one ("Bootstrap failed: 5:
    // Input/output error"); retry instead of leaving the sites down.
    let mut last = String::new();
    for _ in 0..10 {
        let out = Command::new("/bin/launchctl").arg("bootstrap").arg(gui_domain()).arg(&plist).output()?;
        if out.status.success() {
            return Ok(());
        }
        last = String::from_utf8_lossy(&out.stderr).trim().to_string();
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    Err(io::Error::other(format!(
        "launchctl bootstrap failed: {last}; retry with `launchctl bootstrap {} {}`",
        gui_domain(),
        plist.display()
    )))
}

/// Stop the agent (launchd sends SIGTERM; the daemon stops PHP and nginx cleanly) and wait until
/// launchd has really released it: `bootout` returns before teardown finishes, and a `bootstrap`
/// right after it fails (this took the owner's sites down during the first upgrade).
pub fn stop_service() {
    let _ = Command::new("/bin/launchctl").arg("bootout").arg(format!("{}/{AGENT_LABEL}", gui_domain())).output();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(super::EXIT_TIMEOUT + 5);
    while service_loaded() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// Stop the agent and delete its plist, so nothing starts the daemon at the next login.
pub fn remove_service() {
    stop_service();
    let _ = fs::remove_file(service_path());
}

pub fn service_loaded() -> bool {
    Command::new("/bin/launchctl")
        .arg("print")
        .arg(format!("{}/{AGENT_LABEL}", gui_domain()))
        .output()
        .is_ok_and(|o| o.status.success())
}

/// The System keychain already serves every browser on macOS.
pub fn browser_trust_needed() -> bool {
    false
}

pub fn browser_trust(_ca: &Path) -> Option<Result<String, String>> {
    None
}

pub fn browser_untrust() -> Option<Result<String, String>> {
    None
}

/// Checks only macOS needs.
pub fn os_checks(_paths: &Paths) -> Vec<Check> {
    let valet = Command::new("pgrep").args(["-f", "/usr/local/opt/dnsmasq"]).output().is_ok_and(|o| o.status.success());
    vec![(
        !valet,
        "Valet is stopped".into(),
        "Valet still runs (its dnsmasq); once Devid works, stop it with `valet stop`".into(),
    )]
}

/// Opens a Terminal window in `dir`.
pub fn terminal_command(dir: &str) -> Result<Command, String> {
    let mut cmd = Command::new("open");
    cmd.args(["-a", "Terminal"]).arg(dir);
    Ok(cmd)
}

/// Every web server runs on macOS (Apache through the system httpd).
pub fn unavailable_server(_name: &str) -> Option<String> {
    None
}

/// Whether a process of group `pgid` has an established TCP connection on its local port `port`:
/// a request in progress, or an open WebSocket such as a dev server's HMR, which an open tab
/// keeps. Read through libproc, without starting a process.
#[cfg(target_os = "macos")]
pub fn established(pgid: i32, port: u16) -> bool {
    libproc::group_pids(pgid).into_iter().any(|pid| {
        libproc::fds(pid)
            .iter()
            .filter(|fd| fd.proc_fdtype == libc::PROX_FDTYPE_SOCKET as u32)
            .any(|fd| libproc::tcp_established_on(pid, fd.proc_fd, port))
    })
}

/// libproc exists only on macOS; this copy keeps the module building elsewhere.
#[cfg(not(target_os = "macos"))]
pub fn established(_pgid: i32, _port: u16) -> bool {
    false
}

#[cfg(target_os = "macos")]
mod libproc {
    use std::{mem::size_of, ptr::null_mut};

    // From <sys/proc_info.h>, the same on arm64 and x86_64 (offsetof checked with clang for both,
    // 2026-11-21). libc has no `socket_fdinfo`, so the few fields needed are read by offset.
    const PROC_PIDFDSOCKETINFO: i32 = 3;
    const SOCKET_FDINFO_SIZE: usize = 792;
    /// psi.soi_kind
    const SOI_KIND: usize = 256;
    /// psi.soi_proto.pri_tcp.tcpsi_ini.insi_lport, in network byte order
    const INSI_LPORT: usize = 268;
    /// psi.soi_proto.pri_tcp.tcpsi_state
    const TCPSI_STATE: usize = 344;
    const SOCKINFO_TCP: i32 = 2;
    const TSI_S_ESTABLISHED: i32 = 4;

    /// The processes of a process group.
    pub fn group_pids(pgid: i32) -> Vec<i32> {
        let mut capacity = 64;
        loop {
            let mut pids = vec![0i32; capacity];
            let bytes = (capacity * size_of::<i32>()) as i32;
            // SAFETY: the buffer holds `bytes` bytes; the call returns how many pids it wrote.
            let n = unsafe { libc::proc_listpgrppids(pgid, pids.as_mut_ptr().cast(), bytes) };
            let Ok(n) = usize::try_from(n) else { return Vec::new() };
            if n < capacity || capacity >= 16_384 {
                pids.truncate(n.min(capacity));
                return pids;
            }
            capacity *= 4;
        }
    }

    /// A process's open file descriptors.
    pub fn fds(pid: i32) -> Vec<libc::proc_fdinfo> {
        // SAFETY: a null buffer asks for the size needed.
        let needed = unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, null_mut(), 0) };
        let Ok(needed) = usize::try_from(needed) else { return Vec::new() };
        // Room for a few descriptors opened since.
        let count = needed / size_of::<libc::proc_fdinfo>() + 16;
        // SAFETY: proc_fdinfo is two integers; all zeroes is a valid value.
        let mut fds: Vec<libc::proc_fdinfo> = (0..count).map(|_| unsafe { std::mem::zeroed() }).collect();
        let bytes = (count * size_of::<libc::proc_fdinfo>()) as i32;
        // SAFETY: the buffer holds `bytes` bytes; the call returns how many it wrote.
        let got = unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, fds.as_mut_ptr().cast(), bytes) };
        let Ok(got) = usize::try_from(got) else { return Vec::new() };
        fds.truncate(got / size_of::<libc::proc_fdinfo>());
        fds
    }

    /// Whether `fd` of `pid` is an established TCP connection on local port `port`.
    pub fn tcp_established_on(pid: i32, fd: i32, port: u16) -> bool {
        let mut info = [0u8; SOCKET_FDINFO_SIZE];
        // SAFETY: the buffer holds a whole socket_fdinfo; the call returns how many bytes it wrote.
        let got = unsafe {
            libc::proc_pidfdinfo(pid, fd, PROC_PIDFDSOCKETINFO, info.as_mut_ptr().cast(), SOCKET_FDINFO_SIZE as i32)
        };
        if usize::try_from(got) != Ok(SOCKET_FDINFO_SIZE) {
            return false;
        }
        let int = |at: usize| i32::from_ne_bytes(info[at..at + 4].try_into().expect("four bytes"));
        int(SOI_KIND) == SOCKINFO_TCP
            && int(TCPSI_STATE) == TSI_S_ESTABLISHED
            && u16::from_be(int(INSI_LPORT) as u16) == port
    }
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
            ca_cert: "/Users/me/.devid/ca/devid-ca.crt".into(),
        }
    }

    #[test]
    fn generated_files() {
        let p = plan();
        assert_eq!(
            resolver_content(&p),
            "# Devid: send .test lookups to the Devid daemon\nnameserver 127.0.0.1\nport 53535\n"
        );
        let rules = pf_rules(&p);
        assert!(rules.contains("to 127.0.0.1 port 80 -> 127.0.0.1 port 8088"));
        assert!(rules.contains("to 127.0.0.1 port 443 -> 127.0.0.1 port 8443"));
        assert!(rules.contains("on lo0"), "local traffic only");
        assert!(pf_plist().contains("<string>dev.devid.pf</string>"));
        // D-047: install also undoes another pf client's `skip on lo0`, which an anchor reload cannot.
        let unskip = install_actions(&p).into_iter().any(|a| {
            matches!(a, Action::Run { ref args, ok_to_fail: true, .. }
                if args.iter().any(|x| x.contains("lo0 (skip)") && x.contains("pfctl -O -f /etc/pf.conf")))
        });
        assert!(unskip, "install clears skip on lo0");
        super::super::tests::uninstall_removes_every_file(&install_actions(&p), &uninstall_actions(&p));
    }

    #[test]
    fn zsh_gets_zshenv_and_zshrc_bash_gets_bash_profile() {
        let home = Path::new("/Users/me");
        assert_eq!(
            rc_files_for("/bin/zsh", home),
            [home.join(".zshenv"), home.join(".zshrc")],
            ".zshenv for every zsh, .zshrc to come before Homebrew after path_helper"
        );
        assert_eq!(rc_files_for("", home), [home.join(".zshenv"), home.join(".zshrc")], "zsh is the default");
        assert_eq!(rc_files_for("/bin/bash", home), [home.join(".bash_profile")]);
    }

    #[test]
    fn agent_plist_runs_the_installed_binary() {
        let paths = Paths { home: PathBuf::from("/Users/me/.devid") };
        let plist = agent_plist(&paths);
        assert!(plist.contains("<string>/Users/me/.devid/bin/devid</string>"));
        assert!(plist.contains("<string>dev.devid.daemon</string>"));
        assert!(plist.contains("<key>SuccessfulExit</key>\n        <false/>"), "no restart after a clean stop");
        assert!(plist.contains("/Users/me/.devid/log/daemon.log"));
        assert!(!plist.contains("<key>ProcessType</key>"), "sites must not run at background priority");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn an_open_connection_is_seen() {
        super::super::tests::sees_an_open_connection(established);
    }
}
