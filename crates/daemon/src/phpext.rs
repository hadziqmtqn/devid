//! Catalog extensions (Phase 6 E3, D-021): PHP extensions from the release manifest's `extensions`
//! section, unpacked under `~/.devid/php-ext/<ver>/<name>/` and turned on by
//! `conf.d/devid-<name>.ini`. They live outside `~/.devid/php/<ver>/`, which a PHP install replaces.

use crate::{
    paths::Paths,
    php,
    releases::{self, Artifact},
};
use serde_json::Value;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

fn err(msg: impl Into<String>) -> io::Error {
    io::Error::other(msg.into())
}

/// A catalog entry for one PHP version, from the manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub name: String,
    pub version: String,
    /// "extension" or "zend_extension"
    pub kind: String,
    pub php_api: String,
    pub license: String,
    /// Archive size in bytes, from the manifest (0 if missing); for progress only.
    pub size: u64,
    pub artifact: Artifact,
}

/// An installed catalog extension.
#[derive(Debug, Clone, PartialEq)]
pub struct Installed {
    pub name: String,
    pub version: String,
    /// SHA-256 of the archive it came from, to spot a rebuild of the same version.
    pub sha256: String,
}

/// Names become paths and ini file names, so only plain extension names are accepted.
pub fn check_name(name: &str) -> io::Result<()> {
    let ok = !name.is_empty() && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    if ok { Ok(()) } else { Err(err(format!("{name:?} is not an extension name"))) }
}

pub fn root(paths: &Paths, version: &str) -> PathBuf {
    paths.home.join("php-ext").join(version)
}

fn ini(paths: &Paths, version: &str, name: &str) -> PathBuf {
    php::etc(paths, version).join("conf.d").join(format!("devid-{name}.ini"))
}

/// The catalog for PHP `version` (e.g. "8.4"), sorted by name; entries without a build for this Mac's
/// architecture or with an unusable name are left out.
pub fn catalog(manifest: &Value, version: &str) -> Vec<Entry> {
    let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "x86_64" };
    let mut out: Vec<Entry> = manifest["extensions"][version]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(name, _)| check_name(name).is_ok())
        .filter_map(|(name, e)| {
            let a = &e["arch"][arch];
            let s = |v: &Value| v.as_str().map(str::to_string);
            let kind = s(&e["type"]).filter(|k| k == "extension" || k == "zend_extension")?;
            Some(Entry {
                name: name.clone(),
                version: s(&e["version"])?,
                kind,
                php_api: s(&e["php_api"])?,
                license: s(&e["license"]).unwrap_or_default(),
                size: a["size"].as_u64().unwrap_or(0),
                artifact: Artifact {
                    tool: format!("ext-{name}-php{version}"),
                    version: s(&e["version"])?,
                    url: s(&a["url"])?,
                    sha256: s(&a["sha256"])?.to_lowercase(),
                    size: a["size"].as_u64().unwrap_or(0),
                },
            })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Installed catalog extensions for `version`, sorted by name.
pub fn installed(paths: &Paths, version: &str) -> Vec<Installed> {
    let mut out: Vec<Installed> = fs::read_dir(root(paths, version))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok().filter(|n| check_name(n).is_ok())?;
            let info = read_json(&e.path().join("ext.json"))?;
            Some(Installed {
                version: info["version"].as_str()?.to_string(),
                sha256: fs::read_to_string(e.path().join(".sha256")).unwrap_or_default().trim().to_string(),
                name,
            })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

/// Environment the installed extensions of `version` need (imagick: MAGICK_CONFIGURE_PATH), with
/// paths made absolute. Set for php-fpm (pool `env[]` and master) and by the `php` shim.
pub fn env(paths: &Paths, version: &str) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    for ext in installed(paths, version) {
        let dir = root(paths, version).join(&ext.name);
        let Some(info) = read_json(&dir.join("ext.json")) else { continue };
        for (k, v) in info["env"].as_object().into_iter().flatten() {
            // Only plain variable names and paths inside the extension's own folder.
            let name_ok = !k.is_empty() && k.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_');
            if let (true, Some(rel)) = (name_ok, v.as_str().filter(|r| !r.starts_with('/') && !r.contains(".."))) {
                out.push((k.clone(), dir.join(rel)));
            }
        }
    }
    out
}

/// The module API of the installed PHP `version`, from its compiled extension dir
/// (".../no-debug-non-zts-20240924"). `php -i` would be simpler but too large for `probe`.
pub fn php_api(paths: &Paths, version: &str) -> io::Result<String> {
    let bin = php::dir(paths, version).join("bin/php");
    let out = crate::webserver::probe(&bin, &["-n", "-r", "echo PHP_EXTENSION_DIR;"])?;
    let dir = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let api: String =
        dir.chars().rev().take_while(char::is_ascii_digit).collect::<Vec<_>>().into_iter().rev().collect();
    if out.status.success() && api.len() == 8 {
        Ok(api)
    } else {
        Err(err(format!("could not read the module API of PHP {version} (got {dir:?})")))
    }
}

/// `php -m` and `php-fpm -m` of `version` with its real php.ini and conf.d, as sites and the shim
/// run it. Ok when every name in `want` is listed and PHP printed no startup warning or error;
/// otherwise the output, for the error message.
pub fn preflight(paths: &Paths, version: &str, want: &[&str]) -> Result<(), String> {
    let etc = php::etc(paths, version);
    let mut env = vec![("PHPRC".to_string(), etc.clone()), ("PHP_INI_SCAN_DIR".to_string(), etc.join("conf.d"))];
    env.extend(self::env(paths, version));
    let ini = etc.join("php.ini").to_string_lossy().into_owned();
    for (bin, args) in [("php", vec!["-m"]), ("php-fpm", vec!["-c", ini.as_str(), "-m"])] {
        let program = php::dir(paths, version).join("bin").join(bin);
        let out = crate::webserver::probe_env(&program, &args, &env).map_err(|e| e.to_string())?;
        let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        let listed = |name: &str| text.lines().any(|l| l.trim().eq_ignore_ascii_case(name));
        let problem =
            text.lines().any(|l| l.contains("Warning") || l.contains("Fatal error") || l.contains("PHP Startup"));
        if !out.status.success() || problem || !want.iter().all(|n| listed(n)) {
            let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('[')).collect();
            let shown = lines.iter().filter(|l| l.contains("Warning") || l.contains("rror") || l.contains("Startup"));
            let shown: Vec<&str> = shown.copied().take(5).collect();
            return Err(format!("{bin} -m: {}", if shown.is_empty() { lines.join(", ") } else { shown.join("\n") }));
        }
    }
    Ok(())
}

fn ini_body(version: &str, name: &str, kind: &str, so: &Path) -> String {
    // apcu is off in the CLI by default; turn it on so `php artisan` and tests see the same cache API.
    let extra = if name == "apcu" { "apc.enable_cli=1\n" } else { "" };
    format!(
        "; Generated by Devid (`devid php ext remove {version} {name}` removes it)\n{kind}={}\n{extra}",
        so.display()
    )
}

/// Module names from `php -m`, once each: Zend extensions such as OPcache appear under both
/// "[PHP Modules]" and "[Zend Modules]".
pub fn module_names(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('[')) {
        if !out.iter().any(|n| n == name) {
            out.push(name.to_string());
        }
    }
    out
}

/// Download, verify and turn on `name` for PHP `version` from `manifest`; returns its version.
/// Nothing changes unless `php -m` and `php-fpm -m` load it cleanly: on failure the previous state
/// (files and ini) comes back. `downloaded` runs once the archive is verified, before the checks
/// (for progress). The caller restarts php-fpm.
pub fn install(
    paths: &Paths,
    version: &str,
    name: &str,
    manifest: &Value,
    downloaded: &dyn Fn(),
) -> io::Result<String> {
    php::check_supported(version)?;
    check_name(name)?;
    if !php::installed(paths).iter().any(|v| v == version) {
        return Err(err(format!("PHP {version} is not installed; run `devid php install {version}`")));
    }
    let entry = catalog(manifest, version).into_iter().find(|e| e.name == name).ok_or_else(|| {
        err(format!("{name} is not in the catalog for PHP {version}; see `devid php ext available {version}`"))
    })?;
    let api = php_api(paths, version)?;
    if entry.php_api != api {
        return Err(err(format!(
            "{name} {} is built for PHP API {}, but the installed PHP {version} has {api}; run `devid update` first",
            entry.version, entry.php_api
        )));
    }

    let unpacked = releases::download(paths, &entry.artifact)?;
    downloaded();
    let result = place(paths, version, &entry, &unpacked);
    let _ = fs::remove_dir_all(&unpacked);
    result?;
    Ok(entry.version)
}

/// Move a verified download into place and turn it on, or put everything back.
fn place(paths: &Paths, version: &str, entry: &Entry, unpacked: &Path) -> io::Result<()> {
    let name = &entry.name;
    let info = read_json(&unpacked.join("ext.json")).ok_or_else(|| err(format!("{name}: archive has no ext.json")))?;
    let file = format!("{name}.so");
    if info["name"] != name.as_str() || info["file"] != file.as_str() || !unpacked.join(&file).is_file() {
        return Err(err(format!("{name}: the archive does not match its catalog entry")));
    }

    let base = root(paths, version);
    fs::create_dir_all(&base)?;
    let dest = base.join(name);
    let backup = base.join(format!(".{name}.old"));
    let ini = ini(paths, version, name);
    let old_ini = fs::read_to_string(&ini).ok();
    let _ = fs::remove_dir_all(&backup);
    if dest.exists() {
        fs::rename(&dest, &backup)?;
    }
    let restore = |why: String| -> io::Error {
        let _ = fs::remove_dir_all(&dest);
        if backup.exists() {
            let _ = fs::rename(&backup, &dest);
        }
        let _ = match &old_ini {
            Some(body) => fs::write(&ini, body),
            None => fs::remove_file(&ini),
        };
        err(why)
    };

    fs::rename(unpacked, &dest).map_err(|e| restore(e.to_string()))?;
    fs::write(dest.join(".sha256"), &entry.artifact.sha256).map_err(|e| restore(e.to_string()))?;
    if let Some(dir) = ini.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&ini, ini_body(version, name, &entry.kind, &dest.join(&file))).map_err(|e| restore(e.to_string()))?;
    if let Err(output) = preflight(paths, version, &[name]) {
        return Err(restore(format!("{name} does not load in PHP {version}, so nothing changed.\n{output}")));
    }
    let _ = fs::remove_dir_all(&backup);
    php::write_config(paths, version)
}

/// Turn `name` off for PHP `version` and delete its files. The caller restarts php-fpm.
pub fn remove(paths: &Paths, version: &str, name: &str) -> io::Result<()> {
    php::check_supported(version)?;
    check_name(name)?;
    let dest = root(paths, version).join(name);
    if !dest.exists() {
        return Err(err(format!("{name} is not installed for PHP {version}")));
    }
    match fs::remove_file(ini(paths, version, name)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    fs::remove_dir_all(&dest)?;
    php::write_config(paths, version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_paths(tag: &str) -> Paths {
        let home = std::env::temp_dir().join(format!("devid-phpext-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        Paths { home }
    }

    #[test]
    fn names_are_plain() {
        assert!(check_name("pdo_sqlsrv").is_ok());
        assert!(check_name("imagick2").is_ok());
        for bad in ["", "Imagick", "../x", "a/b", "a.b", "a b", "a-b"] {
            assert!(check_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn catalog_reads_the_manifest() {
        let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "x86_64" };
        let a = json!({arch: {"url": "u", "sha256": "AB", "size": 2048}});
        let m = json!({"extensions": {"8.4": {
            "mongodb": {"version": "2.5.5", "type": "extension", "php_api": "20240924", "license": "Apache-2.0", "arch": a},
            "apcu": {"version": "5.1.28", "type": "extension", "php_api": "20240924", "arch": a},
            "bad_type": {"version": "1", "type": "module", "php_api": "20240924", "arch": a},
            "../evil": {"version": "1", "type": "extension", "php_api": "20240924", "arch": a},
            "other_arch": {"version": "1", "type": "extension", "php_api": "20240924", "arch": {"sparc": {}}}
        }}});
        let c = catalog(&m, "8.4");
        let names: Vec<&str> = c.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["apcu", "mongodb"]);
        assert_eq!(c[1].artifact.sha256, "ab");
        assert_eq!(c[1].artifact.tool, "ext-mongodb-php8.4");
        assert_eq!(c[1].license, "Apache-2.0");
        assert_eq!(c[1].size, 2048);
        assert!(catalog(&m, "8.5").is_empty());
        assert!(catalog(&json!({}), "8.4").is_empty());
    }

    #[test]
    fn installed_and_env() {
        let paths = temp_paths("env");
        let dir = root(&paths, "8.4").join("imagick");
        fs::create_dir_all(&dir).unwrap();
        let info = json!({"name": "imagick", "version": "3.8.1",
            "env": {"MAGICK_CONFIGURE_PATH": "etc", "BAD-NAME": "etc", "ABS": "/etc", "UP": "../x"}});
        fs::write(dir.join("ext.json"), info.to_string()).unwrap();
        fs::write(dir.join(".sha256"), "abc\n").unwrap();
        fs::create_dir_all(root(&paths, "8.4").join(".apcu.old")).unwrap(); // a leftover backup is not an extension
        let i = installed(&paths, "8.4");
        assert_eq!(i, [Installed { name: "imagick".into(), version: "3.8.1".into(), sha256: "abc".into() }]);
        assert_eq!(env(&paths, "8.4"), [("MAGICK_CONFIGURE_PATH".to_string(), dir.join("etc"))]);
        assert!(env(&paths, "8.5").is_empty());
        let _ = fs::remove_dir_all(&paths.home);
    }

    #[test]
    fn ini_points_at_the_installed_file() {
        let body = ini_body("8.4", "apcu", "extension", Path::new("/h/php-ext/8.4/apcu/apcu.so"));
        assert!(body.contains("\nextension=/h/php-ext/8.4/apcu/apcu.so\n"));
        assert!(body.contains("devid php ext remove 8.4 apcu"));
        assert!(body.ends_with("\napc.enable_cli=1\n"), "apcu also works in the CLI");
        let body = ini_body("8.4", "mongodb", "extension", Path::new("/m.so"));
        assert!(!body.contains("apc."), "only apcu gets apcu settings");
    }

    #[test]
    fn module_names_are_listed_once() {
        let out = "[PHP Modules]\nCore\nZend OPcache\nzip\n\n[Zend Modules]\nZend OPcache\n";
        assert_eq!(module_names(out), ["Core", "Zend OPcache", "zip"]);
    }
}
