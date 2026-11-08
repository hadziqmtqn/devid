//! `~/.devid/config.toml`: the owner's choices. Created with defaults on first start.

use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, io, path::Path};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Top-level domain for sites (`<name>.test`).
    pub tld: String,
    /// Active web server: nginx (default, D-015), caddy or apache.
    pub server: String,
    /// PHP version for sites without their own (D-009: 8.3, 8.4, 8.5).
    pub default_php: String,
    // shortcut: high port until the M5 helper redirects 80/443.
    /// Port the web server listens on (127.0.0.1 only).
    pub http_port: u16,
    /// HTTPS port (127.0.0.1 only), with certificates from the Devid CA.
    pub https_port: u16,
    /// UDP port of the daemon's DNS responder for `.<tld>` (127.0.0.1 only).
    pub dns_port: u16,
    /// Folders whose subfolders each become `<name>.<tld>` (D-011).
    pub parked: Vec<String>,
    /// Explicit sites: name -> folder. A link wins over a parked folder of the same name.
    pub links: BTreeMap<String, String>,
    /// Per-site PHP version: site name -> version.
    pub isolated: BTreeMap<String, String>,
    /// Folders of parked sites the owner removed (D-029): not served; the folders stay as they are.
    pub ignored: Vec<String>,
    /// Start nginx and php-fpm when the daemon starts, at login (D-031). Off: they wait for
    /// `devid start all`, also after the daemon restarts.
    pub autostart: bool,
    /// Minutes between automatic update checks (D-028); 0 turns them off.
    pub update_check_minutes: u32,
    /// Per-site changes to the detected app type (D-035): site name -> overrides.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub apps: BTreeMap<String, App>,
}

/// What the owner changed about one site's app type (D-035). A missing field falls back to
/// detection, so `[apps.<site>]` holds only what differs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct App {
    /// A preset key from `sites::PRESETS` (laravel, vite, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    /// php, static or proxy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// Web root for php and static, relative to the project folder ("." is the folder).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    /// Port of the dev server a proxy site forwards to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream: Option<u16>,
    /// Static sites: unknown paths load /index.html.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spa: Option<bool>,
    /// Proxy sites: send the .test Host and Origin unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pass_host: Option<bool>,
    /// Also serve the site on http://127.0.0.1:<port>, for a tunnel or a tool (D-037).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Commands the owner starts and stops from Devid (D-036), in the order they were added.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<AppCommand>,
}

/// One saved command: `[[apps.<site>.commands]]` with `name` and `run` (a shell line).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AppCommand {
    pub name: String,
    pub run: String,
}

impl App {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tld: "test".into(),
            server: "nginx".into(),
            default_php: "8.5".into(),
            http_port: 8088,
            https_port: 8443,
            dns_port: 53535,
            parked: Vec::new(),
            links: BTreeMap::new(),
            isolated: BTreeMap::new(),
            ignored: Vec::new(),
            autostart: true,
            update_check_minutes: 20,
            apps: BTreeMap::new(),
        }
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
                config.save(path)?;
                Ok(config)
            }
            Err(e) => Err(e),
        }
    }

    /// Write atomically (temp file + rename) so a crash never leaves a truncated config.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("toml.tmp");
        fs::write(&tmp, toml::to_string_pretty(self).expect("config serializes"))?;
        fs::rename(tmp, path)
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

        fs::write(&path, "default_php = \"8.4\"\n[apps.shop]\nmode = \"proxy\"\nupstream = 5173\n").unwrap();
        let apps = Config::load_or_create(&path).unwrap();
        let shop = &apps.apps["shop"];
        assert_eq!((shop.mode.as_deref(), shop.upstream, shop.preset.as_deref()), (Some("proxy"), Some(5173), None));
        apps.save(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("[apps.shop]") && !text.contains("preset"), "only the overrides are written: {text}");
        fs::write(&path, "[[apps.erd.commands]]\nname = \"dev:pg:local\"\nrun = \"npm run dev:pg:local\"\n").unwrap();
        let erd = &Config::load_or_create(&path).unwrap().apps["erd"];
        assert_eq!(
            (erd.commands[0].name.as_str(), erd.commands[0].run.as_str()),
            ("dev:pg:local", "npm run dev:pg:local")
        );
        Config::default().save(&path).unwrap();
        assert!(!fs::read_to_string(&path).unwrap().contains("apps"), "no empty [apps] table");

        fs::write(&path, "server = [").unwrap();
        assert!(Config::load_or_create(&path).is_err(), "invalid TOML is an error, not silently replaced");

        fs::remove_dir_all(&dir).unwrap();
    }
}
