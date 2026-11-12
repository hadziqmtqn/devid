//! Sites: parked folders and explicit links resolved into `<name>.<tld>` entries (D-011), each
//! with an app type (D-035): a serving mode filled in by a detected preset, changeable per site.

use crate::config::{App, Config};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

/// What the web server does with a site.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Files from the web root; `.php` goes to the site's php-fpm (the behaviour before D-035).
    #[default]
    Php,
    /// Files from the web root; nothing else runs.
    Static,
    /// Everything goes to a dev server on `localhost:<upstream>`, WebSockets included.
    Proxy,
}

impl Mode {
    pub const NAMES: [&'static str; 3] = ["php", "static", "proxy"];

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "php" => Some(Self::Php),
            "static" => Some(Self::Static),
            "proxy" => Some(Self::Proxy),
            _ => None,
        }
    }
}

/// A framework label that fills in a site's defaults (D-035).
pub struct Preset {
    pub key: &'static str,
    pub label: &'static str,
    pub mode: Mode,
    /// Proxy: the port the framework's dev server uses by default.
    pub upstream: u16,
    /// Static: where the framework's build puts its files.
    pub static_root: &'static str,
}

const fn preset(
    key: &'static str,
    label: &'static str,
    mode: Mode,
    upstream: u16,
    static_root: &'static str,
) -> Preset {
    Preset { key, label, mode, upstream, static_root }
}

/// In detection order (`detect`). "php" is also the fallback when nothing matches.
pub const PRESETS: [Preset; 11] = [
    preset("laravel", "Laravel", Mode::Php, 3000, "."),
    preset("ci4", "CodeIgniter 4", Mode::Php, 3000, "."),
    preset("ci3", "CodeIgniter 3", Mode::Php, 3000, "."),
    preset("symfony", "Symfony", Mode::Php, 3000, "."),
    preset("wordpress", "WordPress", Mode::Php, 3000, "."),
    preset("php", "PHP", Mode::Php, 3000, "."),
    preset("next", "Next.js", Mode::Proxy, 3000, "out"),
    preset("nuxt", "Nuxt", Mode::Proxy, 3000, ".output/public"),
    preset("node", "Node.js", Mode::Proxy, 3000, "."),
    preset("vite", "Vite", Mode::Proxy, 5173, "dist"),
    preset("static", "Static", Mode::Static, 3000, "."),
];

pub fn find_preset(key: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.key == key)
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Site {
    pub name: String,
    pub host: String,
    pub path: PathBuf,
    /// Web root for php and static: the `root` override, else `<path>/public` when it exists
    /// (php) or the preset's build folder (static); `<path>` for proxy.
    pub docroot: PathBuf,
    /// PHP version actually used: the isolated one or the default.
    pub php: String,
    pub isolated: bool,
    pub linked: bool,
    /// Preset key in force: the owner's choice, else the detected one, else "php".
    pub preset: &'static str,
    /// What detection found, whatever the owner chose.
    pub detected: Option<&'static str>,
    pub mode: Mode,
    /// Proxy: dev server port (override, else `PORT=` in `.env`, else the preset's).
    pub upstream: u16,
    /// Static: unknown paths load /index.html.
    pub spa: bool,
    /// Proxy: send the .test Host and Origin unchanged.
    pub pass_host: bool,
    /// Also served on http://127.0.0.1:<port> (D-037). The daemon drops it from the web server's
    /// config while another program listens there.
    pub port: Option<u16>,
}

impl Site {
    /// D-041: the `PORT` a proxy site's commands get, its dev server port ("Forward to").
    pub fn dev_port(&self) -> Option<u16> {
        (self.mode == Mode::Proxy).then_some(self.upstream)
    }
}

/// Keys of the given top-level objects in a JSON file (dependencies, scripts…); empty if unreadable.
fn json_keys(file: &Path, sections: &[&str]) -> BTreeSet<String> {
    let Ok(text) = fs::read_to_string(file) else { return BTreeSet::new() };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else { return BTreeSet::new() };
    sections.iter().filter_map(|s| value[*s].as_object()).flat_map(|o| o.keys().cloned()).collect()
}

/// The preset a project's files point to, checked in `PRESETS` order; None when nothing matches.
/// It only reads marker files and never runs anything from the project.
pub fn detect(dir: &Path) -> Option<&'static str> {
    let has = |f: &str| dir.join(f).exists();
    let composer = json_keys(&dir.join("composer.json"), &["require", "require-dev"]);
    if has("artisan") && composer.contains("laravel/framework") {
        return Some("laravel");
    }
    if has("spark") {
        return Some("ci4");
    }
    if has("system/CodeIgniter.php") || has("system/core/CodeIgniter.php") {
        return Some("ci3");
    }
    if has("bin/console") && composer.contains("symfony/framework-bundle") {
        return Some("symfony");
    }
    if has("wp-config.php") || has("wp-load.php") {
        return Some("wordpress");
    }
    if has("composer.json") || has("index.php") || has("public/index.php") {
        return Some("php");
    }
    if has("package.json") {
        let deps = json_keys(&dir.join("package.json"), &["dependencies", "devDependencies"]);
        let scripts = json_keys(&dir.join("package.json"), &["scripts"]);
        let any = |names: &[&str]| names.iter().any(|n| deps.contains(*n));
        if any(&["next"]) {
            return Some("next");
        }
        if any(&["nuxt"]) {
            return Some("nuxt");
        }
        if any(&["express", "fastify", "koa", "hono"]) {
            return Some("node");
        }
        if any(&["vite"]) {
            return Some("vite");
        }
        if scripts.contains("dev") || scripts.contains("start") {
            return Some("node");
        }
    }
    if has("index.html") {
        return Some("static");
    }
    None
}

/// `PORT=<n>` in the project's `.env`, the port its dev server listens on. Only that line is read.
fn env_port(dir: &Path) -> Option<u16> {
    let text = fs::read_to_string(dir.join(".env")).ok()?;
    text.lines().find_map(|l| {
        let value = l.trim().strip_prefix("export ").unwrap_or(l.trim()).strip_prefix("PORT=")?;
        value.trim().trim_matches(|c| c == '"' || c == '\'').parse().ok().filter(|p| *p > 0)
    })
}

/// A web root override must stay inside the project: relative, without `..`.
/// Commands a site could run (D-036), as (name, shell line): package.json scripts named dev*,
/// start* or watch*, with the package manager its lockfile shows, and Laravel's workers. Only
/// suggestions: nothing runs until the owner saves one and starts it.
pub fn suggest_commands(dir: &Path, preset: &str) -> Vec<(String, String)> {
    let has = |f: &str| dir.join(f).exists();
    let pm = match () {
        _ if has("pnpm-lock.yaml") => "pnpm",
        _ if has("yarn.lock") => "yarn",
        _ if has("bun.lock") || has("bun.lockb") => "bun",
        _ => "npm",
    };
    let mut out: Vec<(String, String)> = json_keys(&dir.join("package.json"), &["scripts"])
        .into_iter()
        .filter(|s| ["dev", "start", "watch"].iter().any(|p| s.starts_with(p)))
        .map(|s| (s.clone(), format!("{pm} run {s}")))
        .collect();
    if preset == "laravel" {
        out.push(("queue".into(), "php artisan queue:work".into()));
        out.push(("schedule".into(), "php artisan schedule:work".into()));
        if json_keys(&dir.join("composer.json"), &["require", "require-dev"]).contains("laravel/reverb") {
            out.push(("reverb".into(), "php artisan reverb:start".into()));
        }
    }
    out
}

pub fn safe_root(root: &str) -> bool {
    !root.is_empty() && Path::new(root).components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

/// The site's app type: the owner's overrides over the detected preset's defaults.
fn app_type(path: &Path, app: Option<&App>) -> (Option<&'static str>, &'static Preset, Mode) {
    let detected = detect(path);
    let chosen = app.and_then(|a| a.preset.as_deref()).and_then(find_preset);
    let preset = chosen
        .or_else(|| detected.and_then(find_preset))
        .unwrap_or_else(|| find_preset("php").expect("the php preset exists"));
    let mode = app.and_then(|a| a.mode.as_deref()).and_then(Mode::parse).unwrap_or(preset.mode);
    (detected, preset, mode)
}

/// DNS label rules, so a folder name can never inject into nginx config or a hostname.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 63
        && !name.starts_with('-')
        && !name.ends_with('-')
        && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A folder name as a site name: lowercased, `_`/`.`/spaces become `-`. None if still invalid.
pub fn name_from_folder(folder: &str) -> Option<String> {
    let name: String =
        folder.to_lowercase().chars().map(|c| if c == '_' || c == '.' || c == ' ' { '-' } else { c }).collect();
    valid_name(&name).then_some(name)
}

/// All sites, sorted by name. Unreadable parked folders and invalid names are skipped, not fatal.
pub fn resolve(config: &Config) -> Vec<Site> {
    let mut found: BTreeMap<String, (PathBuf, bool)> = BTreeMap::new();
    for parked in &config.parked {
        let Ok(entries) = fs::read_dir(parked) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let hidden = entry.file_name().to_string_lossy().starts_with('.');
            let ignored = config.ignored.iter().any(|i| Path::new(i) == path);
            if hidden || ignored || !path.is_dir() {
                continue;
            }
            if let Some(name) = name_from_folder(&entry.file_name().to_string_lossy()) {
                found.entry(name).or_insert((path, false));
            }
        }
    }
    for (name, path) in &config.links {
        found.insert(name.clone(), (PathBuf::from(path), true));
    }

    found
        .into_iter()
        .map(|(name, (path, linked))| {
            let isolated = config.isolated.get(&name);
            let app = config.apps.get(&name);
            let (detected, preset, mode) = app_type(&path, app);
            let root = app.and_then(|a| a.root.as_deref()).filter(|r| safe_root(r));
            let public = path.join("public");
            let docroot = match (mode, root) {
                (Mode::Proxy, _) => path.clone(),
                (_, Some(r)) => path.join(r),
                (Mode::Static, None) => path.join(preset.static_root),
                (Mode::Php, None) if public.is_dir() => public,
                (Mode::Php, None) => path.clone(),
            };
            // `.env` is read only for proxy sites, and only its PORT line.
            let upstream = match mode {
                Mode::Proxy => app.and_then(|a| a.upstream).or_else(|| env_port(&path)).unwrap_or(preset.upstream),
                _ => app.and_then(|a| a.upstream).unwrap_or(preset.upstream),
            };
            Site {
                host: format!("{name}.{}", config.tld),
                // Collecting the components drops "." ("<path>/." becomes "<path>").
                docroot: docroot.components().collect(),
                php: isolated.cloned().unwrap_or_else(|| config.default_php.clone()),
                isolated: isolated.is_some(),
                linked,
                preset: preset.key,
                detected,
                mode,
                upstream,
                spa: app.and_then(|a| a.spa).unwrap_or(preset.key == "vite"),
                pass_host: app.and_then(|a| a.pass_host).unwrap_or(false),
                port: app.and_then(|a| a.port),
                name,
                path,
            }
        })
        .collect()
}

/// The site name an ignored folder had (D-029): its folder name as a site name.
pub fn ignored_name(path: &str) -> String {
    let folder = Path::new(path).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    name_from_folder(&folder).unwrap_or(folder)
}

/// The site whose folder contains `dir` (deepest match), for commands run inside a project.
pub fn find_by_dir<'a>(sites: &'a [Site], dir: &Path) -> Option<&'a Site> {
    sites.iter().filter(|s| dir.starts_with(&s.path)).max_by_key(|s| s.path.components().count())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(valid_name("erdbpro-saas"));
        assert!(!valid_name("bad name"));
        assert!(!valid_name("evil;}"));
        assert!(!valid_name("-x"));
        assert_eq!(name_from_folder("My_App.v2").as_deref(), Some("my-app-v2"));
        assert_eq!(name_from_folder("caf\u{e9}"), None);
    }

    #[test]
    fn resolve_parked_links_and_isolation() {
        let root = std::env::temp_dir().join(format!("devid-sites-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for dir in ["parked/blog/public", "parked/api", "parked/.hidden", "parked/Bad Name!", "elsewhere/shop"] {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        fs::write(root.join("parked/notes.txt"), b"").unwrap();

        let config = Config {
            parked: vec![root.join("parked").display().to_string(), "/does/not/exist".into()],
            links: [
                ("shop".to_string(), root.join("elsewhere/shop").display().to_string()),
                ("api".to_string(), root.join("elsewhere/shop").display().to_string()),
            ]
            .into(),
            isolated: [("blog".to_string(), "8.4".to_string())].into(),
            ..Config::default()
        };
        let sites = resolve(&config);
        let names: Vec<&str> = sites.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["api", "blog", "shop"], "hidden, files and invalid names skipped");

        let blog = &sites[1];
        assert_eq!(blog.host, "blog.test");
        assert_eq!(blog.docroot, root.join("parked/blog/public"));
        assert_eq!((blog.php.as_str(), blog.isolated, blog.linked), ("8.4", true, false));

        let api = &sites[0];
        assert!(api.linked, "a link overrides the parked folder with the same name");
        assert_eq!(api.docroot, root.join("elsewhere/shop"));
        assert_eq!(api.php, "8.5");

        assert_eq!(find_by_dir(&sites, &root.join("parked/blog/public/css")).unwrap().name, "blog");
        assert!(find_by_dir(&sites, &root).is_none());

        // An ignored parked folder is skipped (D-029); a link of the same name still wins.
        let ignored = Config { ignored: vec![root.join("parked/blog").display().to_string()], ..config.clone() };
        let names: Vec<String> = resolve(&ignored).into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["api", "shop"]);
        assert_eq!(ignored_name(&root.join("parked/My_App").display().to_string()), "my-app");

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn app_types_detected_and_overridden() {
        let root = std::env::temp_dir().join(format!("devid-apps-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let file = |rel: &str, text: &str| {
            let p = root.join("p").join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, text).unwrap();
        };
        file("lara/artisan", "");
        file("lara/composer.json", r#"{"require": {"laravel/framework": "^12"}}"#);
        fs::create_dir_all(root.join("p/lara/public")).unwrap();
        file("ci3/system/core/CodeIgniter.php", "");
        file("wp/wp-config.php", "");
        file("pma/composer.json", "{}");
        file("pma/package.json", r#"{"devDependencies": {"vite": "^6"}}"#);
        file("erd/package.json", r#"{"dependencies": {"express": "^5"}, "devDependencies": {"vite": "^8"}}"#);
        file("erd/.env", "APP_URL=https://x\nexport PORT=\"3010\"\n");
        file("vt/package.json", r#"{"devDependencies": {"vite": "^6"}}"#);
        file("nx/package.json", r#"{"dependencies": {"next": "16"}}"#);
        file("docs/package.json", r#"{"scripts": {"dev": "x"}}"#);
        file("lp/index.html", "");
        fs::create_dir_all(root.join("p/lp/public")).unwrap();
        file("rust/Cargo.toml", "");

        let mut config = Config { parked: vec![root.join("p").display().to_string()], ..Config::default() };
        let get = |config: &Config, name: &str| resolve(config).into_iter().find(|s| s.name == name).unwrap();
        let summary = |s: &Site| (s.preset, s.detected, s.mode);
        assert_eq!(summary(&get(&config, "lara")), ("laravel", Some("laravel"), Mode::Php));
        assert_eq!(get(&config, "lara").docroot, root.join("p/lara/public"));
        assert_eq!(summary(&get(&config, "ci3")), ("ci3", Some("ci3"), Mode::Php));
        assert_eq!(summary(&get(&config, "wp")), ("wordpress", Some("wordpress"), Mode::Php));
        assert_eq!(
            summary(&get(&config, "pma")),
            ("php", Some("php"), Mode::Php),
            "composer.json wins over package.json"
        );
        let erd = get(&config, "erd");
        assert_eq!((summary(&erd), erd.upstream), (("node", Some("node"), Mode::Proxy), 3010), "PORT from .env");
        assert_eq!(erd.docroot, root.join("p/erd"));
        let vt = get(&config, "vt");
        assert_eq!((summary(&vt), vt.upstream, vt.spa), (("vite", Some("vite"), Mode::Proxy), 5173, true));
        assert_eq!(summary(&get(&config, "nx")), ("next", Some("next"), Mode::Proxy));
        assert_eq!(summary(&get(&config, "docs")), ("node", Some("node"), Mode::Proxy));
        let lp = get(&config, "lp");
        assert_eq!((summary(&lp), lp.docroot.clone()), (("static", Some("static"), Mode::Static), root.join("p/lp")));
        let rust = get(&config, "rust");
        assert_eq!(
            (summary(&rust), rust.docroot.clone()),
            (("php", None, Mode::Php), root.join("p/rust")),
            "as before"
        );

        // Overrides: a mode keeps the preset; a preset brings its own mode; a root may not leave the project.
        config.apps.insert("vt".into(), App { mode: Some("static".into()), ..App::default() });
        config
            .apps
            .insert("erd".into(), App { preset: Some("vite".into()), root: Some("../x".into()), ..App::default() });
        config.apps.insert("lp".into(), App { root: Some("./public".into()), spa: Some(true), ..App::default() });
        let vt = get(&config, "vt");
        assert_eq!(
            (vt.preset, vt.mode, vt.docroot.clone(), vt.spa),
            ("vite", Mode::Static, root.join("p/vt/dist"), true)
        );
        let erd = get(&config, "erd");
        assert_eq!((erd.preset, erd.detected, erd.mode, erd.upstream), ("vite", Some("node"), Mode::Proxy, 3010));
        let lp = get(&config, "lp");
        assert_eq!((lp.docroot.clone(), lp.spa), (root.join("p/lp/public"), true));

        assert!(safe_root(".") && safe_root("public") && safe_root("./dist/app"));
        assert!(!safe_root("") && !safe_root("../x") && !safe_root("/etc") && !safe_root("a/../../b"));

        // Command suggestions (D-036): dev/start/watch scripts with the lockfile's package manager.
        file("erd/package.json", r#"{"scripts": {"dev:pg:local": "x", "build": "x", "start": "x"}}"#);
        file("erd/pnpm-lock.yaml", "");
        let erd: Vec<String> =
            suggest_commands(&root.join("p/erd"), "node").into_iter().map(|(n, r)| format!("{n}={r}")).collect();
        assert_eq!(erd, ["dev:pg:local=pnpm run dev:pg:local", "start=pnpm run start"]);
        let lara: Vec<String> = suggest_commands(&root.join("p/lara"), "laravel").into_iter().map(|(n, _)| n).collect();
        assert_eq!(lara, ["queue", "schedule"], "reverb only when the app requires laravel/reverb");
        assert!(suggest_commands(&root.join("p/rust"), "php").is_empty());
        fs::remove_dir_all(&root).unwrap();
    }
}
