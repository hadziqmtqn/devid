//! A tiny DNS responder: every `<name>.<tld>` resolves to 127.0.0.1 (replaces Valet's dnsmasq).
//! `/etc/resolver/<tld>` (installed by the helper) sends only `.<tld>` queries here.

use std::{io, net::UdpSocket, thread};

const TYPE_A: u16 = 1;
const CLASS_IN: u16 = 1;
const RCODE_FORMERR: u8 = 1;
const RCODE_REFUSED: u8 = 5;

/// Bind `127.0.0.1:<port>` and answer queries on a background thread.
pub fn spawn(port: u16, tld: String) -> io::Result<()> {
    let socket = UdpSocket::bind(("127.0.0.1", port))?;
    thread::spawn(move || {
        let mut buf = [0u8; 512];
        loop {
            let Ok((len, peer)) = socket.recv_from(&mut buf) else { continue };
            if let Some(reply) = answer(&buf[..len], &tld) {
                let _ = socket.send_to(&reply, peer);
            }
        }
    });
    Ok(())
}

/// The reply for one query packet, or None for packets that are not a query at all.
/// A for `.<tld>` names -> 127.0.0.1; other types for those names -> no data (so clients use
/// IPv4, where nginx listens); names outside the TLD -> REFUSED.
pub fn answer(query: &[u8], tld: &str) -> Option<Vec<u8>> {
    if query.len() < 12 || query[2] & 0x80 != 0 {
        return None; // too short, or a response rather than a query
    }
    let qdcount = u16::from_be_bytes([query[4], query[5]]);
    let opcode = (query[2] >> 3) & 0x0f;
    let Some((name, end)) = (qdcount == 1 && opcode == 0).then(|| question(query)).flatten() else {
        return Some(header(query, RCODE_FORMERR, 0, 0));
    };
    let qtype = u16::from_be_bytes([query[end - 4], query[end - 3]]);
    let qclass = u16::from_be_bytes([query[end - 2], query[end - 1]]);

    let tld = tld.to_ascii_lowercase();
    let ours = name == tld || name.ends_with(&format!(".{tld}"));
    if !ours {
        let mut reply = header(query, RCODE_REFUSED, 1, 0);
        reply.extend_from_slice(&query[12..end]);
        return Some(reply);
    }
    let a = qtype == TYPE_A && qclass == CLASS_IN;
    let mut reply = header(query, 0, 1, a as u16);
    reply.extend_from_slice(&query[12..end]);
    if a {
        reply.extend_from_slice(&[0xc0, 0x0c]); // pointer to the name in the question
        reply.extend_from_slice(&TYPE_A.to_be_bytes());
        reply.extend_from_slice(&CLASS_IN.to_be_bytes());
        reply.extend_from_slice(&60u32.to_be_bytes()); // TTL
        reply.extend_from_slice(&4u16.to_be_bytes());
        reply.extend_from_slice(&[127, 0, 0, 1]);
    }
    Some(reply)
}

/// The question's lowercased name and the offset just past QTYPE/QCLASS. Bounds-checked;
/// compression pointers are not allowed in a question.
fn question(packet: &[u8]) -> Option<(String, usize)> {
    let mut pos = 12;
    let mut labels: Vec<String> = Vec::new();
    loop {
        let len = *packet.get(pos)? as usize;
        pos += 1;
        if len == 0 {
            break;
        }
        if len > 63 {
            return None;
        }
        let label = packet.get(pos..pos + len)?;
        labels.push(String::from_utf8_lossy(label).to_ascii_lowercase());
        pos += len;
    }
    let end = pos + 4;
    (end <= packet.len()).then(|| (labels.join("."), end))
}

/// Response header: same id and RD bit, QR + AA set, given RCODE and counts.
fn header(query: &[u8], rcode: u8, qdcount: u16, ancount: u16) -> Vec<u8> {
    let mut h = Vec::with_capacity(64);
    h.extend_from_slice(&query[0..2]);
    h.push(0x80 | 0x04 | (query[2] & 0x79)); // QR, AA, keep opcode + RD
    h.push(rcode);
    h.extend_from_slice(&qdcount.to_be_bytes());
    h.extend_from_slice(&ancount.to_be_bytes());
    h.extend_from_slice(&[0, 0, 0, 0]); // NSCOUNT, ARCOUNT
    h
}

#[cfg(test)]
pub(crate) fn build_query(id: u16, name: &str, qtype: u16) -> Vec<u8> {
    let mut q = id.to_be_bytes().to_vec();
    q.extend_from_slice(&[0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0]); // RD, QDCOUNT=1
    for label in name.split('.') {
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.push(0);
    q.extend_from_slice(&qtype.to_be_bytes());
    q.extend_from_slice(&CLASS_IN.to_be_bytes());
    q
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_for_tld_names() {
        let reply = answer(&build_query(0xbeef, "ErdbPro-SaaS.Test", TYPE_A), "test").unwrap();
        assert_eq!(&reply[0..2], &[0xbe, 0xef], "same id");
        assert_eq!(reply[3] & 0x0f, 0, "NOERROR");
        assert_eq!(u16::from_be_bytes([reply[6], reply[7]]), 1, "one answer");
        assert_eq!(&reply[reply.len() - 4..], &[127, 0, 0, 1]);
        // Bare TLD and deep subdomains too.
        assert!(answer(&build_query(1, "test", TYPE_A), "test").unwrap().ends_with(&[127, 0, 0, 1]));
        assert!(answer(&build_query(1, "a.b.test", TYPE_A), "test").unwrap().ends_with(&[127, 0, 0, 1]));
    }

    #[test]
    fn aaaa_is_empty_and_others_refused() {
        let aaaa = answer(&build_query(2, "blog.test", 28), "test").unwrap();
        assert_eq!((aaaa[3] & 0x0f, aaaa[7]), (0, 0), "NOERROR with no answers");
        let other = answer(&build_query(3, "example.com", TYPE_A), "test").unwrap();
        assert_eq!(other[3] & 0x0f, RCODE_REFUSED);
        let tricky = answer(&build_query(4, "nottest", TYPE_A), "test").unwrap();
        assert_eq!(tricky[3] & 0x0f, RCODE_REFUSED, "suffix match is on a label boundary");
    }

    #[test]
    fn malformed_packets_are_safe() {
        assert!(answer(&[0u8; 5], "test").is_none());
        let mut truncated = build_query(5, "blog.test", TYPE_A);
        truncated.truncate(20);
        assert_eq!(answer(&truncated, "test").unwrap()[3] & 0x0f, RCODE_FORMERR);
        let mut bad_label = build_query(6, "blog.test", TYPE_A);
        bad_label[12] = 0xc0; // compression pointer in the question
        assert_eq!(answer(&bad_label, "test").unwrap()[3] & 0x0f, RCODE_FORMERR);
        let mut response = build_query(7, "blog.test", TYPE_A);
        response[2] |= 0x80;
        assert!(answer(&response, "test").is_none(), "never answer a response (no reflection loops)");
    }
}
