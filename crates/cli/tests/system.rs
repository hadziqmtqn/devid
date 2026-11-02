//! M5 without root: the daemon's DNS responder over real UDP, and the helper's dry run and
//! root guard. The root-only `helper apply` itself is verified manually (it changes the system).

mod common;

use common::*;
use std::{fs, net::UdpSocket, time::Duration};

fn query(port: u16, name: &str) -> Vec<u8> {
    let mut q = vec![0x12, 0x34, 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0];
    for label in name.split('.') {
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.extend_from_slice(&[0, 0, 1, 0, 1]); // root, type A, class IN
    let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
    sock.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    sock.send_to(&q, ("127.0.0.1", port)).unwrap();
    let mut buf = [0u8; 512];
    let (len, _) = sock.recv_from(&mut buf).expect("DNS reply");
    buf[..len].to_vec()
}

#[test]
fn dns_and_helper_without_root() {
    let root = temp_root("devid-system-it");
    let home = root.join("home");
    let port = free_udp_port();
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join("config.toml"),
        format!("dns_port = {port}\nhttp_port = {TEST_HTTP_PORT}\nhttps_port = {TEST_HTTPS_PORT}\n"),
    )
    .unwrap();

    let daemon = start_daemon(&home);
    let status = json(&home, &["status", "--json"]);
    assert_eq!(status["dns"], format!("listening on 127.0.0.1:{port}"));
    assert_eq!(status["standard_ports"], false);

    let reply = query(port, "erdbpro-saas.test");
    assert_eq!(&reply[0..2], &[0x12, 0x34]);
    assert_eq!(reply[3] & 0x0f, 0, "NOERROR");
    assert_eq!(&reply[reply.len() - 4..], &[127, 0, 0, 1]);
    assert_eq!(query(port, "example.com")[3] & 0x0f, 5, "REFUSED outside .test");

    // The dry run lists every system change and changes nothing.
    let out = devid(&home, &["helper", "install", "--dry-run"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let plan = String::from_utf8_lossy(&out.stdout);
    for expected in ["/etc/resolver/test", "pfctl", "add-trusted-cert", "dry run: nothing changed"] {
        assert!(plan.contains(expected), "dry run mentions {expected}:\n{plan}");
    }
    let out = devid(&home, &["helper", "uninstall", "--dry-run"]);
    assert!(String::from_utf8_lossy(&out.stdout).contains("remove-trusted-cert"));

    // The root step refuses to run without root.
    let ca = home.join("ca/devid-ca.crt");
    let out = devid(
        &home,
        &[
            "helper",
            "apply",
            "install",
            "--tld",
            "test",
            "--dns-port",
            "53535",
            "--http-port",
            "8088",
            "--https-port",
            "8443",
            "--ca",
            ca.to_str().unwrap(),
        ],
    );
    assert!(!out.status.success());
    assert!(stderr(&out).contains("must run as root"));

    daemon.stop();
    fs::remove_dir_all(&root).unwrap();
}
