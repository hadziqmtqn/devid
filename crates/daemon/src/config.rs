//! `~/.devid/config.toml`: the owner's choices. Created with defaults on first start.

use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Top-level domain for sites (`<name>.test`).
    pub tld: String,
    /// Active web server: nginx (default, D-015), caddy or apache.
    pub server: String,
    /// PHP version for sites without their own (D-009: 8.3, 8.4, 8.5).
    pub default_php: String,
}

impl Default for Config {
    fn default() -> Self {
        Self { tld: "test".into(), server: "nginx".into(), default_php: "8.5".into() }
    }
}

impl Config {
    /// Load the config, writing the defaults first if the file does not exist yet.
    pub fn load_or_create(path: &Path) -> io::Result<Self> {
        match fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{}: {e}", path.display()))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let config = Self::default();
                if let Some(dir) = path.parent() {
                    fs::create_dir_all(dir)?;
                }
                fs::write(path, toml::to_string_pretty(&config).expect("config serializes"))?;
                Ok(config)
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_defaults_then_reads_them_back() {
        let dir = std::env::temp_dir().join(format!("devid-config-test-{}", std::process::id()));
        let path = dir.join("config.toml");
        let _ = fs::remove_dir_all(&dir);

        let created = Config::load_or_create(&path).unwrap();
        assert_eq!(created, Config::default());
        assert!(path.exists());

        fs::write(&path, "default_php = \"8.4\"\n").unwrap();
        let edited = Config::load_or_create(&path).unwrap();
        assert_eq!(edited.default_php, "8.4");
        assert_eq!(edited.server, "nginx", "missing keys fall back to defaults");

        fs::write(&path, "server = [").unwrap();
        assert!(Config::load_or_create(&path).is_err(), "invalid TOML is an error, not silently replaced");

        fs::remove_dir_all(&dir).unwrap();
    }
}
