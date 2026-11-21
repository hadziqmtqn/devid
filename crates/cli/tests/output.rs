//! Output to a reader that went away (`devid sites | head -1`): the CLI ends quietly with success
//! instead of panicking on the broken pipe, so such pipelines hold under `set -o pipefail`.

mod common;

use common::*;
use std::{
    path::Path,
    process::{Command, Output, Stdio},
};

/// Run devid with stdout on a pipe whose read end is already closed, so its first write gets EPIPE.
fn to_closed_pipe(home: &Path, args: &[&str]) -> Output {
    let (reader, writer) = std::io::pipe().unwrap();
    drop(reader);
    Command::new(BIN).args(args).env("DEVID_HOME", home).stdout(writer).stderr(Stdio::piped()).output().unwrap()
}

#[test]
fn closed_stdout_ends_quietly() {
    let root = temp_root("devid-output-it");
    let home = root.join("home");
    let daemon = start_daemon(&home);
    for args in [&["status"][..], &["status", "--json"], &["sites"], &["php", "list"]] {
        // Each of these prints something, so the broken pipe is hit.
        assert!(!devid(&home, args).stdout.is_empty(), "devid {args:?} prints nothing");
        let out = to_closed_pipe(&home, args);
        assert!(out.status.success(), "devid {args:?} exited with {}: {}", out.status, stderr(&out));
        assert_eq!(stderr(&out), "", "devid {args:?}");
    }
    daemon.stop();
    let _ = std::fs::remove_dir_all(&root);
}
