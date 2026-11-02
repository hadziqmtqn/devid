//! Sites: parked folders and explicit links resolved into `<name>.<tld>` entries (D-011).

use crate::config::Config;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Site {
    pub name: String,
    pub host: String,
    pub path: PathBuf,
    /// `<path>/public` when it exists (Laravel), otherwise `<path>`.
    pub docroot: PathBuf,
    /// PHP version actually used: the isolated one or the default.
    pub php: String,
    pub isolated: bool,
    pub linked: bool,
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
            if hidden || !path.is_dir() {
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
            let public = path.join("public");
            Site {
                host: format!("{name}.{}", config.tld),
                docroot: if public.is_dir() { public } else { path.clone() },
                php: isolated.cloned().unwrap_or_else(|| config.default_php.clone()),
                isolated: isolated.is_some(),
                linked,
                name,
                path,
            }
        })
        .collect()
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

        fs::remove_dir_all(&root).unwrap();
    }
}
