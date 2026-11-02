//! The Devid local CA and per-site certificates (D-008, risk 3). Nothing here touches the system
//! trust store; trusting the CA is a separate, explicit step (M5 helper).

use crate::paths::Paths;
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, GeneralSubtree, IsCa,
    Issuer, KeyPair, KeyUsagePurpose, NameConstraints, PKCS_ECDSA_P256_SHA256, SerialNumber,
};
use std::{
    fs,
    io::{self, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use time::{Duration, OffsetDateTime};

const CA_NAME: &str = "Devid Local CA";
/// Total lifetime, counted from not_before. Apple rejects locally trusted TLS certificates over
/// 825 days (and public ones over 398, shrinking further); 397 stays clear of both, and leaves
/// renew automatically.
const LEAF_DAYS: i64 = 397;
/// Renew a leaf this long before it expires.
const RENEW_BEFORE_DAYS: i64 = 30;
const CA_DAYS: i64 = 3650;

fn err(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}

pub fn dir(paths: &Paths) -> PathBuf {
    paths.home.join("ca")
}

pub fn ca_cert(paths: &Paths) -> PathBuf {
    dir(paths).join("devid-ca.crt")
}

fn ca_key(paths: &Paths) -> PathBuf {
    dir(paths).join("devid-ca.key")
}

/// The TLD the current CA is constrained to; a different TLD (or none recorded) means a new CA.
fn ca_tld_file(paths: &Paths) -> PathBuf {
    dir(paths).join("devid-ca.tld")
}

pub fn site_cert(paths: &Paths, host: &str) -> PathBuf {
    paths.home.join("certs").join(format!("{host}.crt"))
}

pub fn site_key(paths: &Paths, host: &str) -> PathBuf {
    paths.home.join("certs").join(format!("{host}.key"))
}

/// The CA's identity. Rebuilt the same way every time, so the stored key alone is enough to
/// sign new leaves after a restart.
///
/// Name constraints limit the CA to `<tld>` and its subdomains: once the owner trusts it, a
/// leaked key still cannot impersonate any real site (e.g. a bank) on this Mac.
fn ca_params(tld: &str) -> CertificateParams {
    let mut params = CertificateParams::default();
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, CA_NAME);
    dn.push(DnType::OrganizationName, "Devid");
    params.distinguished_name = dn;
    params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign, KeyUsagePurpose::DigitalSignature];
    params.name_constraints = Some(NameConstraints {
        permitted_subtrees: vec![GeneralSubtree::DnsName(tld.to_string())],
        excluded_subtrees: Vec::new(),
    });
    params
}

/// A unique serial per certificate; browsers reject two different certificates with the same
/// issuer and serial.
fn serial() -> SerialNumber {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    SerialNumber::from_slice(&nanos.to_be_bytes()[4..])
}

/// Write a private key readable by the owner only, created with mode 0600 (never briefly wider).
fn write_secret(path: &Path, pem: &str) -> io::Result<()> {
    fs::create_dir_all(path.parent().expect("key has a parent"))?;
    let tmp = path.with_extension("tmp");
    let mut file = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
    file.write_all(pem.as_bytes())?;
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
    fs::rename(tmp, path)
}

/// Create the CA once, constrained to `tld`. Returns true if it was created now.
/// A CA made for another TLD, or before constraints existed, is replaced.
pub fn ensure_ca(paths: &Paths, tld: &str) -> io::Result<bool> {
    let same_tld = fs::read_to_string(ca_tld_file(paths)).is_ok_and(|t| t.trim() == tld);
    if ca_cert(paths).is_file() && ca_key(paths).is_file() && same_tld {
        return Ok(false);
    }
    let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).map_err(err)?;
    let mut params = ca_params(tld);
    let now = OffsetDateTime::now_utc();
    params.not_before = now - Duration::days(1);
    params.not_after = now + Duration::days(CA_DAYS);
    params.serial_number = Some(serial());
    let cert = params.self_signed(&key).map_err(err)?;
    write_secret(&ca_key(paths), &key.serialize_pem())?;
    fs::write(ca_cert(paths), cert.pem())?;
    fs::write(ca_tld_file(paths), tld)?;
    // Leaves signed by an older CA would no longer verify.
    let _ = fs::remove_dir_all(paths.home.join("certs"));
    Ok(true)
}

/// Make sure `host` has a certificate from the current CA that is not about to expire.
/// Returns true if one was (re)issued.
pub fn ensure_site_cert(paths: &Paths, host: &str) -> io::Result<bool> {
    let (cert_path, key_path) = (site_cert(paths, host), site_key(paths, host));
    if cert_path.is_file() && key_path.is_file() && !expiring(&cert_path) {
        return Ok(false);
    }
    let ca_key = KeyPair::from_pem(&fs::read_to_string(ca_key(paths))?).map_err(err)?;
    // Signing only needs the issuer's DN and key; the TLD here does not affect the leaf.
    let issuer = Issuer::new(ca_params("test"), ca_key);

    let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).map_err(err)?;
    let mut params = CertificateParams::new(vec![host.to_string()]).map_err(err)?;
    params.distinguished_name.push(DnType::CommonName, host);
    params.is_ca = IsCa::ExplicitNoCa;
    params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    params.use_authority_key_identifier_extension = true;
    // Backdate an hour for clock skew; the lifetime counts from not_before (826 days once failed).
    let not_before = OffsetDateTime::now_utc() - Duration::hours(1);
    params.not_before = not_before;
    params.not_after = not_before + Duration::days(LEAF_DAYS);
    params.serial_number = Some(serial());
    let cert = params.signed_by(&key, &issuer).map_err(err)?;

    write_secret(&key_path, &key.serialize_pem())?;
    fs::write(&cert_path, cert.pem())?;
    fs::write(expiry_file(&cert_path), params.not_after.unix_timestamp().to_string())?;
    Ok(true)
}

/// Expiry is recorded next to the cert at issue time, so checking it needs no X.509 parser.
fn expiry_file(cert: &Path) -> PathBuf {
    cert.with_extension("expires")
}

fn expiring(cert: &Path) -> bool {
    let Some(expires) = fs::read_to_string(expiry_file(cert)).ok().and_then(|s| s.trim().parse::<i64>().ok()) else {
        return true;
    };
    expires - OffsetDateTime::now_utc().unix_timestamp() < RENEW_BEFORE_DAYS * 86_400
}

/// System CA bundles: macOS first; the Linux path keeps the test suite runnable in CI.
const SYSTEM_BUNDLES: [&str; 2] = ["/etc/ssl/cert.pem", "/etc/ssl/certs/ca-certificates.crt"];

pub fn cacert(paths: &Paths) -> PathBuf {
    dir(paths).join("cacert.pem")
}

/// D-008: PHP trusts the system bundle plus the Devid CA. Rewritten only when its content changes.
pub fn write_cacert(paths: &Paths) -> io::Result<()> {
    let source =
        SYSTEM_BUNDLES.iter().find(|p| Path::new(p).is_file()).ok_or_else(|| err("no system CA bundle found"))?;
    let mut bundle = fs::read_to_string(source)?;
    if !bundle.ends_with('\n') {
        bundle.push('\n');
    }
    bundle.push_str(&format!("\n# {CA_NAME}\n"));
    bundle.push_str(&fs::read_to_string(ca_cert(paths))?);
    if fs::read_to_string(cacert(paths)).is_ok_and(|old| old == bundle) {
        return Ok(());
    }
    let tmp = cacert(paths).with_extension("tmp");
    fs::write(&tmp, bundle)?;
    fs::rename(tmp, cacert(paths))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn ca_and_site_certificates_verify() {
        let home = std::env::temp_dir().join(format!("devid-ca-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        let paths = Paths { home: home.clone() };

        assert!(ensure_ca(&paths, "test").unwrap(), "created on first call");
        assert!(!ensure_ca(&paths, "test").unwrap(), "kept on the next call");
        let mode = fs::metadata(ca_key(&paths)).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "CA key is private");

        assert!(ensure_site_cert(&paths, "blog.test").unwrap());
        assert!(!ensure_site_cert(&paths, "blog.test").unwrap(), "not reissued while valid");
        assert_eq!(fs::metadata(site_key(&paths, "blog.test")).unwrap().permissions().mode() & 0o777, 0o600);

        // An independent verifier: the leaf chains to the CA and is valid for its host and TLS servers.
        let out = Command::new("openssl")
            .args(["verify", "-purpose", "sslserver", "-CAfile"])
            .arg(ca_cert(&paths))
            .arg(site_cert(&paths, "blog.test"))
            .output()
            .expect("openssl is installed");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stdout) + String::from_utf8_lossy(&out.stderr)
        );
        let text = Command::new("openssl")
            .args(["x509", "-noout", "-text", "-in"])
            .arg(site_cert(&paths, "blog.test"))
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&text.stdout);
        assert!(text.contains("DNS:blog.test"), "SAN present");
        assert!(text.contains("TLS Web Server Authentication"), "serverAuth EKU present");

        // Name constraints: the CA is only valid for *.test, so a leaf for a real domain fails
        // verification even though the same CA signed it.
        let ca_text =
            Command::new("openssl").args(["x509", "-noout", "-text", "-in"]).arg(ca_cert(&paths)).output().unwrap();
        assert!(String::from_utf8_lossy(&ca_text.stdout).contains("Permitted:"), "name constraints present");
        ensure_site_cert(&paths, "www.example.com").unwrap();
        let out = Command::new("openssl")
            .args(["verify", "-purpose", "sslserver", "-CAfile"])
            .arg(ca_cert(&paths))
            .arg(site_cert(&paths, "www.example.com"))
            .output()
            .unwrap();
        assert!(!out.status.success(), "a non-.test leaf must not verify");

        // An expiring leaf is reissued.
        fs::write(expiry_file(&site_cert(&paths, "blog.test")), "0").unwrap();
        assert!(ensure_site_cert(&paths, "blog.test").unwrap());

        // Changing the TLD replaces the CA (and drops the old leaves).
        assert!(ensure_ca(&paths, "localhost").unwrap());
        assert!(!site_cert(&paths, "blog.test").exists());
        ensure_ca(&paths, "test").unwrap();

        // cacert.pem = system bundle + the Devid CA.
        write_cacert(&paths).unwrap();
        let bundle = fs::read_to_string(cacert(&paths)).unwrap();
        assert!(bundle.ends_with(&fs::read_to_string(ca_cert(&paths)).unwrap()));

        fs::remove_dir_all(&home).unwrap();
    }
}
