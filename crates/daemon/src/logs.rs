//! Log files in `~/.devid/log/`: list, read the tail (or what is new since an offset), clear.
//! Only plain `*.log` names inside that folder are accepted, so a request cannot reach other files.
//! A user command's log (D-036) may have `:` in its name, as npm scripts do (`dev:pg:local`).

use crate::paths::Paths;
use serde_json::{Value, json};
use std::{
    fs,
    io::{self, Read, Seek, SeekFrom},
    path::PathBuf,
};

/// How much a first read returns: the end of the file, enough for a screen or `-n` lines.
const TAIL_BYTES: u64 = 64 * 1024;

fn err(msg: impl Into<String>) -> io::Error {
    io::Error::other(msg.into())
}

fn path_of(paths: &Paths, name: &str) -> io::Result<PathBuf> {
    let ok = name.ends_with(".log")
        && !name.starts_with('.')
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'));
    if !ok {
        return Err(err(format!("{name:?} is not a log name (see `devid logs`)")));
    }
    let path = paths.log_dir().join(name);
    if !path.is_file() {
        return Err(err(format!("no log named {name}")));
    }
    Ok(path)
}

/// Every `*.log` in the log folder with its size, sorted by name.
pub fn list(paths: &Paths) -> Vec<Value> {
    let mut logs: Vec<(String, u64)> = fs::read_dir(paths.log_dir())
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    let meta = e.metadata().ok()?;
                    (name.ends_with(".log") && meta.is_file()).then_some((name, meta.len()))
                })
                .collect()
        })
        .unwrap_or_default();
    logs.sort();
    logs.into_iter().map(|(name, size)| json!({"name": name, "size": size})).collect()
}

/// Text from `offset` to the end (or the last TAIL_BYTES when `offset` is None), and the new end
/// offset for the next call. A file that shrank (cleared or rotated) is read from the start.
pub fn read(paths: &Paths, name: &str, offset: Option<u64>) -> io::Result<(String, u64)> {
    let mut file = fs::File::open(path_of(paths, name)?)?;
    let len = file.metadata()?.len();
    let start = match offset {
        Some(o) if o <= len => o,
        Some(_) => 0,
        None => len.saturating_sub(TAIL_BYTES),
    };
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.take(len - start).read_to_end(&mut bytes)?;
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if offset.is_none() && start > 0 {
        // Started mid-file: drop the partial first line.
        text = text.split_once('\n').map(|(_, rest)| rest.to_string()).unwrap_or_default();
    }
    Ok((text, len))
}

pub fn clear(paths: &Paths, name: &str) -> io::Result<()> {
    // Truncate in place: the writer (nginx, php-fpm, the daemon) keeps its open file.
    fs::OpenOptions::new().write(true).truncate(true).open(path_of(paths, name)?).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_read_clear() {
        let home = std::env::temp_dir().join(format!("devid-logs-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        let paths = Paths { home: home.clone() };
        fs::create_dir_all(paths.log_dir()).unwrap();
        fs::write(paths.log_dir().join("nginx-error.log"), "one\ntwo\n").unwrap();
        fs::write(paths.log_dir().join("notes.txt"), "x").unwrap();

        assert_eq!(list(&paths), vec![json!({"name": "nginx-error.log", "size": 8})], "only *.log");
        let (text, end) = read(&paths, "nginx-error.log", None).unwrap();
        assert_eq!((text.as_str(), end), ("one\ntwo\n", 8));
        fs::write(paths.log_dir().join("nginx-error.log"), "one\ntwo\nthree\n").unwrap();
        assert_eq!(read(&paths, "nginx-error.log", Some(end)).unwrap(), ("three\n".into(), 14), "only what is new");

        clear(&paths, "nginx-error.log").unwrap();
        assert_eq!(read(&paths, "nginx-error.log", Some(14)).unwrap(), (String::new(), 0), "shrunk: from the start");

        for bad in ["../config.toml", "/etc/passwd", "notes.txt", ".hidden.log", "a/b.log", "missing.log"] {
            assert!(read(&paths, bad, None).is_err(), "{bad} refused");
            assert!(clear(&paths, bad).is_err(), "{bad} refused");
        }
        fs::remove_dir_all(&home).unwrap();
    }
}
