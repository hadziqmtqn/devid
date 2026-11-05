//! Downloads from Devid's public binary releases (D-016, Phase 5 R3): read `manifest.json`,
//! fetch an artifact, verify its SHA-256 and unpack it. Nothing is installed from a URL the
//! manifest's own repository did not publish, and nothing unverified is unpacked.

use serde_json::Value;
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

pub const MANIFEST_URL: &str = "https://github.com/hadziqmtqn/devid-binaries/releases/latest/download/manifest.json";
const TRUSTED_PREFIX: &str = "https://github.com/hadziqmtqn/devid-binaries/releases/download/";

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
    std::env::var("DEVID_RELEASE_BASE")
        .ok()
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| "https://github.com/hadziqmtqn/devid-binaries/releases/download".into())
}

/// Download `url` to `dest` with curl, HTTPS only (file:// only when a test points Devid at a local
/// release through DEVID_MANIFEST_URL or DEVID_RELEASE_BASE).
pub fn fetch(url: &str, dest: &Path) -> io::Result<()> {
    let local_release = manifest_url().starts_with("file://") || release_base().starts_with("file://");
    let proto = if url.starts_with("file://") && local_release { "=file" } else { "=https" };
    let out = Command::new("curl")
        .args(["-fsSL", "--retry", "2", "--proto", proto, "-o"])
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

/// "8.4.27" > "8.4.9", numerically; anything unparsable counts as older.
pub fn newer(candidate: &str, installed: &str) -> bool {
    let parse = |v: &str| v.split('.').map(|p| p.parse::<u64>().ok()).collect::<Option<Vec<u64>>>();
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
}
