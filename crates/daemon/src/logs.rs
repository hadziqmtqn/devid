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
/// Terminal escape sequences are taken out; `\r` stays, for the reader to redraw a progress line.
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
    // What the writer was still in the middle of stays for the next read.
    bytes.truncate(whole(&bytes));
    let end = start + bytes.len() as u64;
    let mut text = strip_escapes(&String::from_utf8_lossy(&bytes));
    if offset.is_none() && start > 0 {
        // Started mid-file: drop the partial first line.
        text = text.split_once('\n').map(|(_, rest)| rest.to_string()).unwrap_or_default();
    }
    Ok((text, end))
}

/// Text without terminal escape sequences (colors, cursor moves, window titles): cargo, vite and
/// npm write them even when their output goes to a file.
fn strip_escapes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\x1b' {
            out.push(c);
            continue;
        }
        match chars.next() {
            // CSI: parameters, then one final byte.
            Some('[') => {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            // OSC: up to BEL or ESC \.
            Some(']') => {
                while let Some(c) = chars.next() {
                    if c == '\x07' || (c == '\x1b' && chars.next() == Some('\\')) {
                        break;
                    }
                }
            }
            // ESC ( B and the like.
            Some(' '..='/') => {
                chars.next();
            }
            _ => {}
        }
    }
    out
}

/// How many bytes are whole: a UTF-8 character or an escape sequence cut off at the end is left
/// out, so the next read gets it in one piece.
fn whole(bytes: &[u8]) -> usize {
    let mut end = bytes.len();
    if let Some(esc) = bytes.iter().rposition(|&b| b == 0x1b) {
        let seq = &bytes[esc..];
        let done = match seq.get(1) {
            None => false,
            Some(b'[') => seq[2..].iter().any(|b| (0x40..=0x7E).contains(b)),
            Some(b']') => seq.contains(&0x07) || seq.windows(2).skip(1).any(|w| w == b"\x1b\\"),
            Some(b' '..=b'/') => seq.len() > 2,
            Some(_) => true,
        };
        // A long run with no end is not a sequence being written; let it through.
        if !done && seq.len() < 64 {
            end = esc;
        }
    }
    // The last character's lead byte, at most 3 bytes back.
    if let Some(back) = (1..=end.min(4)).find(|&b| bytes[end - b] & 0xC0 != 0x80) {
        let lead = bytes[end - back];
        let need = match lead {
            0xF0.. => 4,
            0xE0.. => 3,
            0xC0.. => 2,
            _ => 1,
        };
        if need > back {
            end -= back;
        }
    }
    end
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

        // Colors and a progress line from cargo; an escape and a UTF-8 character cut at the end wait.
        let cargo = "\x1b[1m\x1b[96m    Building\x1b[0m [=> ] 1/2\r\x1b[K\x1b[1m\x1b[92m    Finished\x1b[0m\n\x1b]0;title\x07ok \u{2026}\n";
        fs::write(paths.log_dir().join("nginx-error.log"), [cargo.as_bytes(), b"half \xE2\x80", b"\x1b[1"].concat())
            .unwrap();
        let (text, end) = read(&paths, "nginx-error.log", Some(0)).unwrap();
        assert_eq!(text, "    Building [=> ] 1/2\r    Finished\nok \u{2026}\nhalf ");
        assert_eq!(end as usize, cargo.len() + 5, "stops before the cut character");
        fs::write(
            paths.log_dir().join("nginx-error.log"),
            [cargo.as_bytes(), "half \u{2026}\x1b[1mbold\n".as_bytes()].concat(),
        )
        .unwrap();
        assert_eq!(read(&paths, "nginx-error.log", Some(end)).unwrap().0, "\u{2026}bold\n", "then the rest");
        assert_eq!(strip_escapes("a\x1b(Bb\x1b=c"), "abc");

        for bad in ["../config.toml", "/etc/passwd", "notes.txt", ".hidden.log", "a/b.log", "missing.log"] {
            assert!(read(&paths, bad, None).is_err(), "{bad} refused");
            assert!(clear(&paths, bad).is_err(), "{bad} refused");
        }
        fs::remove_dir_all(&home).unwrap();
    }
}
