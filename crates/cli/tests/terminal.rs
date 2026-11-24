//! D-053: Devid's commands on PATH for new terminals. The app's Add to PATH goes through the
//! daemon's `shell_path`; `devid path` does the same without it. Both write only to the HOME the
//! test gives them.
mod common;
use common::*;
use std::fs;

#[test]
fn add_to_path_from_the_daemon_and_the_cli() {
    let root = temp_root("devid-terminal-it");
    let home = root.join("home");
    let user = root.join("user");
    fs::create_dir_all(&user).unwrap();
    let env = [("HOME", user.to_str().unwrap())];
    let daemon = start_daemon_env(&home, &env);
    // Every CLI run gets the test HOME too: `devid path` writes the startup files itself.
    let cli = |args: &[&str]| {
        std::process::Command::new(BIN).args(args).env("DEVID_HOME", &home).env("HOME", &user).output().unwrap()
    };
    let line = format!("export PATH=\"{}:$PATH\" # added by devid", home.join("bin").display());

    let t = &json(&home, &["status", "--json"])["terminal"];
    assert_eq!(t["on"], false, "{t}");
    assert!(t["commands"].as_array().unwrap().iter().any(|c| c == "php8.4"), "{t}");
    let files: Vec<String> =
        t["files"].as_array().unwrap().iter().map(|f| f["file"].as_str().unwrap().into()).collect();
    assert!(!files.is_empty() && files.iter().all(|f| f.starts_with(user.to_str().unwrap())), "{files:?}");

    // The app's button: every startup file of the login shell gets the line once.
    let reply = request_json(&home, r#"{"cmd":"shell_path","enabled":true}"#);
    assert_eq!(reply["ok"], true, "{reply}");
    assert_eq!(reply["terminal"]["on"], true, "{reply}");
    assert_eq!(reply["changed"].as_array().unwrap().len(), files.len());
    let again = request_json(&home, r#"{"cmd":"shell_path","enabled":true}"#);
    assert!(again["changed"].as_array().unwrap().is_empty(), "idempotent: {again}");
    for f in &files {
        let text = fs::read_to_string(f).unwrap();
        assert_eq!(text.lines().filter(|l| *l == line).count(), 1, "{f}:\n{text}");
    }
    assert!(String::from_utf8_lossy(&devid(&home, &["status"]).stdout).contains("commands are on PATH"));

    // The CLI, with the user's own lines kept.
    fs::write(&files[0], format!("export FOO=1\n{line}\n")).unwrap();
    let out = cli(&["path", "off"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(fs::read_to_string(&files[0]).unwrap(), "export FOO=1\n");
    assert_eq!(json(&home, &["status", "--json"])["terminal"]["on"], false);
    assert!(String::from_utf8_lossy(&cli(&["path"]).stdout).contains("devid path on"));
    assert!(cli(&["path", "on"]).status.success());
    assert_eq!(json(&home, &["status", "--json"])["terminal"]["on"], true);

    daemon.stop();
    fs::remove_dir_all(&root).unwrap();
}

/// bash on Arch (Phase 13 A1.6): /etc/skel gives every user a ~/.bash_profile, so bash's login
/// shells, the desktop session's among them, never read ~/.profile. `devid path on` writes
/// ~/.bash_profile and ~/.bashrc, and `devid path off` also cleans the ~/.profile line 0.19 left.
#[cfg(not(target_os = "macos"))]
#[test]
fn bash_with_a_bash_profile_gets_its_line_there() {
    let root = temp_root("devid-bash-profile-it");
    let home = root.join("home");
    let user = root.join("user");
    fs::create_dir_all(&user).unwrap();
    let skel_profile = "#\n# ~/.bash_profile\n#\n\n[[ -f ~/.bashrc ]] && . ~/.bashrc\n";
    let skel_rc = "# If not running interactively, don't do anything\n[[ $- != *i* ]] && return\n";
    fs::write(user.join(".bash_profile"), skel_profile).unwrap();
    fs::write(user.join(".bashrc"), skel_rc).unwrap();
    let line = format!("export PATH=\"{}:$PATH\" # added by devid", home.join("bin").display());
    fs::write(user.join(".profile"), format!("{line}\n")).unwrap();
    let cli = |args: &[&str]| {
        std::process::Command::new(BIN)
            .args(args)
            .env("DEVID_HOME", &home)
            .env("HOME", &user)
            .env("DEVID_LOGIN_SHELL", "/usr/bin/bash")
            .output()
            .unwrap()
    };
    let text = |out: &std::process::Output| String::from_utf8_lossy(&out.stdout).into_owned();

    let out = cli(&["path", "on"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(text(&out).contains(".bash_profile"), "{}", text(&out));
    assert_eq!(fs::read_to_string(user.join(".bash_profile")).unwrap(), format!("{skel_profile}{line}\n"));
    assert_eq!(fs::read_to_string(user.join(".bashrc")).unwrap(), format!("{skel_rc}{line}\n"));
    assert!(text(&cli(&["path"])).contains("new terminals get Devid's commands"));

    // A bash login shell that is not interactive, as SDDM starts Plasma, gets Devid's bin first.
    let out = std::process::Command::new("bash")
        .args(["--login", "-c", "echo \"${PATH%%:*}\""])
        .env_clear()
        .env("HOME", &user)
        .env("PATH", "/usr/bin:/bin")
        .output()
        .unwrap();
    assert_eq!(text(&out).trim(), home.join("bin").display().to_string(), "{}", stderr(&out));

    let out = cli(&["path", "off"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(fs::read_to_string(user.join(".bash_profile")).unwrap(), skel_profile);
    assert_eq!(fs::read_to_string(user.join(".bashrc")).unwrap(), skel_rc);
    assert!(!user.join(".profile").exists(), "0.19's ~/.profile held only Devid's line");
    fs::remove_dir_all(&root).unwrap();
}

/// fish as the login shell (D-059): `devid path on` makes Devid's own file in fish's conf.d, on
/// every OS, and `devid path off` deletes it. DEVID_LOGIN_SHELL stands in for the user database.
#[test]
fn fish_gets_a_conf_d_file_of_its_own() {
    let root = temp_root("devid-fish-it");
    let home = root.join("home");
    let user = root.join("user");
    fs::create_dir_all(&user).unwrap();
    let cli = |args: &[&str]| {
        std::process::Command::new(BIN)
            .args(args)
            .env("DEVID_HOME", &home)
            .env("HOME", &user)
            .env("DEVID_LOGIN_SHELL", "/usr/bin/fish")
            .output()
            .unwrap()
    };
    let out_text = |out: &std::process::Output| String::from_utf8_lossy(&out.stdout).into_owned();
    let conf = user.join(".config/fish/conf.d/devid.fish");
    let line = format!("fish_add_path --global --move --path '{}' # added by devid", home.join("bin").display());

    assert!(out_text(&cli(&["path"])).contains("devid path on"));
    let out = cli(&["path", "on"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(out_text(&out).contains("devid.fish"), "{}", out_text(&out));
    assert!(cli(&["path", "on"]).status.success(), "idempotent");
    assert_eq!(fs::read_to_string(&conf).unwrap(), format!("{line}\n"));
    let entries: Vec<_> = fs::read_dir(&user).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(entries, [".config"], "no bash or zsh file for a fish user");
    assert!(out_text(&cli(&["path"])).contains("new terminals get Devid's commands"));

    // A real fish, where there is one, puts Devid's bin first even when PATH has it last.
    let fish = ["/usr/bin/fish", "/opt/homebrew/bin/fish", "/usr/local/bin/fish"]
        .into_iter()
        .find(|f| fs::metadata(f).is_ok());
    if let Some(fish) = fish {
        let out = std::process::Command::new(fish)
            .args(["-c", "echo $PATH[1]"])
            .env("HOME", &user)
            .env_remove("XDG_CONFIG_HOME")
            .env("PATH", format!("/usr/bin:/bin:{}", home.join("bin").display()))
            .output()
            .unwrap();
        assert_eq!(out_text(&out).trim(), home.join("bin").display().to_string(), "{}", stderr(&out));
    }

    let out = cli(&["path", "off"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(!conf.exists(), "Devid's own file goes");
    assert!(out_text(&cli(&["path"])).contains("devid path on"));
    fs::remove_dir_all(&root).unwrap();
}
