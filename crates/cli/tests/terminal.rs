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
