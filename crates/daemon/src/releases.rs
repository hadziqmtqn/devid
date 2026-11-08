//! Downloads from Devid's public binary releases (D-016, Phase 5 R3): read `manifest.json`,
//! fetch an artifact, verify its SHA-256 and unpack it. Nothing is installed from a URL the
//! manifest's own repository did not publish, and nothing unverified is unpacked.

use serde_json::Value;
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

/// D-038: the PHP, nginx and Caddy builds are the rolling `runtime-latest` release; GitHub's
/// "Latest" is the newest devid.
pub const MANIFEST_URL: &str =
    "https://github.com/hadziqmtqn/devid-binaries/releases/download/runtime-latest/manifest.json";
const TRUSTED_PREFIX: &str = "https://github.com/hadziqmtqn/devid-binaries/releases/download/";
const REPO: &str = "https://github.com/hadziqmtqn/devid-binaries";
const RELEASE_BASE: &str = "https://github.com/hadziqmtqn/devid-binaries/releases/download";
/// As a release name: GitHub's "Latest" release, the newest devid that is not a pre-release (D-038).
pub const LATEST: &str = "latest";

fn err(msg: impl Into<String>) -> io::Error {
    io::Error::other(msg.into())
}

/// `DEVID_MANIFEST_URL` overrides the manifest location (tests use a local `file://` release).
pub fn manifest_url() -> String {
    std::env::var("DEVID_MANIFEST_URL").ok().filter(|u| !u.is_empty()).unwrap_or_else(|| MANIFEST_URL.into())
}

/// Artifacts must live where the manifest lives: the official release prefix, or for an override
/// the override's own folder.
fn trusted_prefix(manifest: &str) -> String {
    if manifest == MANIFEST_URL {
        TRUSTED_PREFIX.into()
    } else {
        manifest.rsplit_once('/').map(|(dir, _)| format!("{dir}/")).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Artifact {
    pub tool: String,
    pub version: String,
    pub url: String,
    pub sha256: String,
}

/// Where `devid self-update` and install.sh find the devid binary; `DEVID_RELEASE_BASE` overrides it
/// for tests (`file://`).
pub fn release_base() -> String {
    std::env::var("DEVID_RELEASE_BASE").ok().filter(|u| !u.is_empty()).unwrap_or_else(|| RELEASE_BASE.into())
}

/// A devid release file's URL under `base`. GitHub serves its Latest release at its own path; a
/// test release (`DEVID_RELEASE_BASE`) keeps it in a `latest` folder.
fn devid_url(base: &str, release: &str, name: &str) -> String {
    match release {
        LATEST if base == RELEASE_BASE => format!("{REPO}/releases/latest/download/{name}"),
        _ => format!("{base}/{release}/{name}"),
    }
}

/// Download `url` to `dest` with curl, HTTPS only (file:// only when a test points Devid at a local
/// release through DEVID_MANIFEST_URL or DEVID_RELEASE_BASE).
pub fn fetch(url: &str, dest: &Path) -> io::Result<()> {
    let local_release = manifest_url().starts_with("file://") || release_base().starts_with("file://");
    let proto = if url.starts_with("file://") && local_release { "=file" } else { "=https" };
    curl(url, dest, &["--proto", proto])
}

fn curl(url: &str, dest: &Path, extra: &[&str]) -> io::Result<()> {
    let out = Command::new("curl")
        .args(["-fsSL", "--retry", "2"])
        .args(extra)
        .arg("-o")
        .arg(dest)
        .arg(url)
        .output()
        .map_err(|e| err(format!("curl: {e}")))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(err(format!("download failed: {url}: {}", String::from_utf8_lossy(&out.stderr).trim())))
    }
}

pub fn fetch_manifest(paths: &crate::paths::Paths) -> io::Result<Value> {
    let dir = paths.home.join("downloads");
    fs::create_dir_all(&dir)?;
    let dest = dir.join("manifest.json");
    fetch(&manifest_url(), &dest)?;
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(&dest)?).map_err(|e| err(format!("bad manifest: {e}")))?;
    if manifest["schema"] != 1 {
        return Err(err(format!("unsupported manifest schema {}; update devid", manifest["schema"])));
    }
    Ok(manifest)
}

/// The x86_64/arm64 entry for `tool` ("php" needs `minor`, e.g. "8.4").
pub fn artifact(manifest: &Value, tool: &str, minor: Option<&str>) -> Option<Artifact> {
    let slot = match (tool, minor) {
        ("php", Some(m)) => &manifest["php"][m],
        (_, None) => &manifest[tool],
        _ => return None,
    };
    let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "x86_64" };
    let entry = &slot["arch"][arch];
    Some(Artifact {
        tool: tool.into(),
        version: slot["version"].as_str()?.into(),
        url: entry["url"].as_str()?.into(),
        sha256: entry["sha256"].as_str()?.to_lowercase(),
    })
}

pub fn sha256_hex(path: &Path) -> io::Result<String> {
    let mut ctx = ring::digest::Context::new(&ring::digest::SHA256);
    let mut file = fs::File::open(path)?;
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = io::Read::read(&mut file, &mut buf)?;
        if n == 0 {
            break;
        }
        ctx.update(&buf[..n]);
    }
    Ok(ctx.finish().as_ref().iter().map(|b| format!("{b:02x}")).collect())
}

/// Where `download` writes `artifact`'s archive while it arrives (its size there is the progress).
pub fn archive_path(paths: &crate::paths::Paths, artifact: &Artifact) -> PathBuf {
    paths.home.join("downloads").join(format!("{}-{}.tar.gz", artifact.tool, artifact.version))
}

/// Download, verify and unpack `artifact`; returns the unpacked folder (deleted and recreated per call).
pub fn download(paths: &crate::paths::Paths, artifact: &Artifact) -> io::Result<PathBuf> {
    let prefix = trusted_prefix(&manifest_url());
    if prefix.is_empty() || !artifact.url.starts_with(&prefix) {
        return Err(err(format!("refusing {}: not under {prefix}", artifact.url)));
    }
    if artifact.sha256.len() != 64 || !artifact.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(err(format!("manifest has no valid sha256 for {}", artifact.url)));
    }
    let dir = paths.home.join("downloads");
    fs::create_dir_all(&dir)?;
    let name = format!("{}-{}", artifact.tool, artifact.version);
    let archive = archive_path(paths, artifact);
    fetch(&artifact.url, &archive)?;
    let actual = sha256_hex(&archive)?;
    if actual != artifact.sha256 {
        let _ = fs::remove_file(&archive);
        return Err(err(format!(
            "checksum mismatch for {} (expected {}, got {actual}); not installed",
            artifact.url, artifact.sha256
        )));
    }
    let unpacked = dir.join(&name);
    let _ = fs::remove_dir_all(&unpacked);
    fs::create_dir_all(&unpacked)?;
    let status = Command::new("tar").arg("-xzf").arg(&archive).arg("-C").arg(&unpacked).status()?;
    let _ = fs::remove_file(&archive);
    if !status.success() {
        return Err(err(format!("could not unpack {}", archive.display())));
    }
    Ok(unpacked)
}

/// Download one file from GitHub's Latest release (the newest devid, D-038) into ~/.devid/downloads.
pub fn fetch_latest(paths: &crate::paths::Paths, name: &str) -> io::Result<std::path::PathBuf> {
    fetch_devid(paths, LATEST, name)
}

/// Download one file from a devid release (`LATEST` or `devid-v<version>`). When github.com's
/// download link fails for an official release, the same file comes through the GitHub API (on
/// 2026-11-08 the download links answered 504 from the Southeast Asia edge while the API worked).
pub fn fetch_devid(paths: &crate::paths::Paths, release: &str, name: &str) -> io::Result<std::path::PathBuf> {
    let dir = paths.home.join("downloads");
    fs::create_dir_all(&dir)?;
    let dest = dir.join(format!("self-update-{name}"));
    let base = release_base();
    if let Err(e) = fetch(&devid_url(&base, release, name), &dest) {
        if base != RELEASE_BASE {
            return Err(e);
        }
        fetch_from_api(release, name, &dest).map_err(|api| err(format!("{e}; through the GitHub API: {api}")))?;
    }
    Ok(dest)
}

/// The official releases through GitHub's API (60 requests an hour without a token; one update
/// check is two). An asset's `url` there redirects to the same file as its download link.
const API_RELEASES: &str = "https://api.github.com/repos/hadziqmtqn/devid-binaries/releases";

fn fetch_from_api(release: &str, name: &str, dest: &Path) -> io::Result<()> {
    let get = |url: &str, accept: &str| {
        curl(url, dest, &["--proto", "=https", "--proto-redir", "=https", "-H", &format!("Accept: {accept}")])
    };
    let info_url = match release {
        LATEST => format!("{API_RELEASES}/latest"),
        tag => format!("{API_RELEASES}/tags/{tag}"),
    };
    get(&info_url, "application/vnd.github+json")?;
    let info: Value =
        serde_json::from_str(&fs::read_to_string(dest)?).map_err(|e| err(format!("bad release info: {e}")))?;
    let url = asset_api_url(&info, name).ok_or_else(|| err(format!("{name} is not in release {release}")))?;
    get(&url, "application/octet-stream")
}

/// The API link of the asset `name`, only when it points into the official repository.
fn asset_api_url(release: &Value, name: &str) -> Option<String> {
    let asset = release["assets"].as_array()?.iter().find(|a| a["name"] == name)?;
    asset["url"].as_str().filter(|u| u.starts_with(&format!("{API_RELEASES}/assets/"))).map(String::from)
}

/// The newest released devid version.
pub fn latest_devid(paths: &crate::paths::Paths) -> io::Result<String> {
    let file = fetch_latest(paths, "devid-version.txt")?;
    let version = fs::read_to_string(&file)?.trim().to_string();
    let _ = fs::remove_file(file);
    Ok(version)
}

/// "8.4.27" > "8.4.9", numerically; a release is newer than its own pre-release ("0.11.0" >
/// "0.11.0-rc.1"); anything unparsable counts as older.
pub fn newer(candidate: &str, installed: &str) -> bool {
    let parse = |v: &str| {
        let (numbers, stable) = v.split_once('-').map_or((v, true), |(n, _)| (n, false));
        numbers.split('.').map(|p| p.parse::<u64>().ok()).collect::<Option<Vec<u64>>>().map(|n| (n, stable))
    };
    match (parse(candidate), parse(installed)) {
        (Some(c), Some(i)) => c > i,
        (Some(_), None) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_numerically() {
        assert!(newer("8.4.27", "8.4.9"));
        assert!(!newer("8.4.9", "8.4.27"));
        assert!(!newer("8.4.27", "8.4.27"));
        assert!(newer("8.4.27", "garbage"));
        assert!(!newer("garbage", "8.4.27"));
        assert!(newer("0.11.0", "0.11.0-rc.1") && !newer("0.11.0-rc.1", "0.11.0"));
        assert!(!newer("0.10.1", "0.11.0-rc.1"), "a pre-release is not offered an older release");
    }

    #[test]
    fn latest_is_githubs_latest_release() {
        assert_eq!(
            devid_url(RELEASE_BASE, LATEST, "devid-version.txt"),
            "https://github.com/hadziqmtqn/devid-binaries/releases/latest/download/devid-version.txt"
        );
        assert_eq!(
            devid_url(RELEASE_BASE, "devid-v0.10.1", "checksums.txt"),
            format!("{TRUSTED_PREFIX}devid-v0.10.1/checksums.txt")
        );
        assert_eq!(devid_url("file:///tmp/rel", LATEST, "x"), "file:///tmp/rel/latest/x");
        assert!(MANIFEST_URL.starts_with(TRUSTED_PREFIX));
    }

    #[test]
    fn artifacts_must_come_from_the_release() {
        assert_eq!(trusted_prefix(MANIFEST_URL), TRUSTED_PREFIX);
        assert_eq!(trusted_prefix("file:///tmp/rel/manifest.json"), "file:///tmp/rel/");
        let manifest: Value = serde_json::json!({
            "schema": 1,
            "php": {"8.4": {"version": "8.4.27", "arch": {"x86_64": {"url": "u", "sha256": "AB"}, "arm64": {"url": "u", "sha256": "ab"}}}},
            "nginx": {"version": "1.30.6", "arch": {"x86_64": {"url": "n", "sha256": "cd"}, "arm64": {"url": "n", "sha256": "cd"}}}
        });
        let php = artifact(&manifest, "php", Some("8.4")).unwrap();
        assert_eq!((php.version.as_str(), php.sha256.as_str()), ("8.4.27", "ab"));
        assert_eq!(artifact(&manifest, "nginx", None).unwrap().version, "1.30.6");
        assert!(artifact(&manifest, "php", Some("8.1")).is_none());
        assert!(artifact(&manifest, "caddy", None).is_none());
    }

    #[test]
    fn the_api_fallback_only_follows_official_asset_links() {
        let release = serde_json::json!({"assets": [
            {"name": "checksums.txt", "url": format!("{API_RELEASES}/assets/1")},
            {"name": "devid-darwin-x86_64", "url": "https://api.github.com/repos/someone/else/releases/assets/2"}
        ]});
        assert_eq!(asset_api_url(&release, "checksums.txt").unwrap(), format!("{API_RELEASES}/assets/1"));
        assert!(asset_api_url(&release, "devid-darwin-x86_64").is_none(), "another repository's asset");
        assert!(asset_api_url(&release, "devid-version.txt").is_none(), "not in the release");
        assert!(asset_api_url(&serde_json::json!({"message": "Not Found"}), "checksums.txt").is_none());
    }

    /// Live, against the real release: `cargo test -p devid-daemon -- --ignored api_fallback`.
    #[test]
    #[ignore = "network"]
    fn the_api_fallback_downloads_the_checksummed_binary() {
        let dest = std::env::temp_dir().join(format!("devid-api-test-{}", std::process::id()));
        fetch_from_api("devid-latest", "checksums.txt", &dest).unwrap();
        let sums = fs::read_to_string(&dest).unwrap();
        fetch_from_api("devid-latest", "devid-darwin-x86_64", &dest).unwrap();
        let line = format!("{}  devid-darwin-x86_64", sha256_hex(&dest).unwrap());
        fs::remove_file(&dest).unwrap();
        assert!(sums.lines().any(|l| l == line), "{line} not in\n{sums}");
    }
}
