//! Hand-rolled DNS: minimal A query over TCP/53 (no DNS library, N2).
//!
//! The resolver is the measurement target and is recorded per probe, so the
//! evidence states exactly what was measured (e.g. "query-timeout@1.1.1.1").

use std::net::IpAddr;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::config::Target;
use crate::probe::{Probe, ProbeResult, result};

/// The domain every dns target resolves (fixed by design: one A query).
pub const QUERY_DOMAIN: &str = "example.com";

/// Parsed DNS response (what we care about: rcode + A answers).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsResponse {
    pub rcode: u8,
    pub answers: Vec<IpAddr>,
}

/// Build a minimal A query: ID 0, RD=1, one question.
pub fn build_a_query(domain: &str) -> Vec<u8> {
    let mut q = Vec::with_capacity(64);
    q.extend_from_slice(&0u16.to_be_bytes()); // ID
    q.extend_from_slice(&0x0100u16.to_be_bytes()); // flags: RD=1
    q.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT
    q.extend_from_slice(&0u16.to_be_bytes()); // ANCOUNT
    q.extend_from_slice(&0u16.to_be_bytes()); // NSCOUNT
    q.extend_from_slice(&0u16.to_be_bytes()); // ARCOUNT
    for label in domain.split('.') {
        let b = label.as_bytes();
        q.push(b.len() as u8);
        q.extend_from_slice(b);
    }
    q.push(0); // root
    q.extend_from_slice(&1u16.to_be_bytes()); // QTYPE A
    q.extend_from_slice(&1u16.to_be_bytes()); // QCLASS IN
    q
}

/// rcode → name for detail strings.
pub fn rcode_name(rcode: u8) -> String {
    match rcode {
        0 => "NOERROR".into(),
        1 => "SERVFAIL".into(),
        2 => "NXDOMAIN".into(),
        3 => "NOTIMP".into(),
        4 => "REFUSED".into(),
        n => n.to_string(),
    }
}

/// Skip a domain name (handles compression pointers); returns position after it.
fn skip_name(bytes: &[u8], mut pos: usize) -> Result<usize, String> {
    loop {
        if pos >= bytes.len() {
            return Err("name overflow".into());
        }
        let len = bytes[pos];
        if len == 0 {
            return Ok(pos + 1);
        }
        if len & 0xC0 == 0xC0 {
            return Ok(pos + 2); // compression pointer
        }
        pos += 1 + len as usize;
    }
}

/// Parse a DNS response: header + ANCOUNT A records (question/authority/
/// additional sections are skipped).
pub fn parse_response(bytes: &[u8]) -> Result<DnsResponse, String> {
    if bytes.len() < 12 {
        return Err("short response".into());
    }
    let flags = u16::from_be_bytes([bytes[2], bytes[3]]);
    let rcode = (flags & 0x0F) as u8;
    let qdcount = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
    let ancount = u16::from_be_bytes([bytes[6], bytes[7]]) as usize;
    let mut pos = 12;
    for _ in 0..qdcount {
        pos = skip_name(bytes, pos)?;
        pos += 4; // QTYPE + QCLASS
    }
    let mut answers = Vec::new();
    for _ in 0..ancount {
        pos = skip_name(bytes, pos)?;
        if pos + 10 > bytes.len() {
            return Err("truncated answer".into());
        }
        let typ = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]);
        let rdlength = u16::from_be_bytes([bytes[pos + 8], bytes[pos + 9]]) as usize;
        pos += 10; // TYPE CLASS TTL
        if pos + rdlength > bytes.len() {
            return Err("truncated rdata".into());
        }
        if typ == 1 && rdlength == 4 {
            answers.push(IpAddr::from([
                bytes[pos],
                bytes[pos + 1],
                bytes[pos + 2],
                bytes[pos + 3],
            ]));
        }
        pos += rdlength;
    }
    Ok(DnsResponse { rcode, answers })
}

pub struct DnsProbe {
    pub target: Target,
    pub resolver: IpAddr,
    pub domain: String,
}

impl DnsProbe {
    pub fn new(target: Target, resolver: IpAddr, domain: String) -> Self {
        DnsProbe {
            target,
            resolver,
            domain,
        }
    }
}

#[async_trait]
impl Probe for DnsProbe {
    fn target(&self) -> &Target {
        &self.target
    }

    async fn run(&self) -> ProbeResult {
        let r = self.resolver;
        let dur = Duration::from_secs(self.target.timeout);
        let start = Instant::now();
        let query = build_a_query(&self.domain);
        let len_prefix = (query.len() as u16).to_be_bytes();

        let mut stream = match timeout(dur, TcpStream::connect((r, 53))).await {
            Err(_) => return result(&self.target, false, None, format!("query-timeout@{r}")),
            Ok(Err(_)) => return result(&self.target, false, None, format!("connect@{r}")),
            Ok(Ok(s)) => s,
        };

        let write_res = timeout(dur, async {
            stream.write_all(&len_prefix).await?;
            stream.write_all(&query).await
        })
        .await;
        if write_res.is_err() || write_res.as_ref().is_err() {
            return result(&self.target, false, None, format!("query-timeout@{r}"));
        }

        let mut len_buf = [0u8; 2];
        match timeout(dur, stream.read_exact(&mut len_buf)).await {
            Err(_) => result(&self.target, false, None, format!("query-timeout@{r}")),
            Ok(Err(_)) => result(&self.target, false, None, format!("connect@{r}")),
            Ok(Ok(_)) => {
                let n = u16::from_be_bytes(len_buf) as usize;
                if !(12..=65_535).contains(&n) {
                    return result(&self.target, false, None, format!("bad-response@{r}"));
                }
                let mut buf = vec![0u8; n];
                match timeout(dur, stream.read_exact(&mut buf)).await {
                    Err(_) => result(&self.target, false, None, format!("query-timeout@{r}")),
                    Ok(Err(_)) => result(&self.target, false, None, format!("connect@{r}")),
                    Ok(Ok(_)) => match parse_response(&buf) {
                        Err(e) => {
                            result(&self.target, false, None, format!("bad-response@{r}:{e}"))
                        }
                        Ok(resp) => {
                            if resp.rcode != 0 {
                                result(
                                    &self.target,
                                    false,
                                    None,
                                    format!("rcode={}@{r}", rcode_name(resp.rcode)),
                                )
                            } else if resp.answers.is_empty() {
                                result(&self.target, false, None, format!("no-answers@{r}"))
                            } else {
                                let latency = start.elapsed().as_secs_f64() * 1000.0;
                                result(&self.target, true, Some(latency), "")
                            }
                        }
                    },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic response: header + the example.com question +
    /// optional A answer (name via 0xC00C pointer).
    fn resp_fixture(flags: u16, ancount: u16, rdata: Option<[u8; 4]>) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&0u16.to_be_bytes()); // ID
        b.extend_from_slice(&flags.to_be_bytes());
        b.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT
        b.extend_from_slice(&ancount.to_be_bytes()); // ANCOUNT
        b.extend_from_slice(&0u16.to_be_bytes()); // NSCOUNT
        b.extend_from_slice(&0u16.to_be_bytes()); // ARCOUNT
        b.extend_from_slice(&build_a_query("example.com")[12..]); // question
        if let Some(ip) = rdata {
            b.extend_from_slice(&[0xC0, 0x0C]); // name pointer to the question
            b.extend_from_slice(&1u16.to_be_bytes()); // TYPE A
            b.extend_from_slice(&1u16.to_be_bytes()); // CLASS IN
            b.extend_from_slice(&300u32.to_be_bytes()); // TTL
            b.extend_from_slice(&4u16.to_be_bytes()); // RDLENGTH
            b.extend_from_slice(&ip);
        }
        b
    }

    #[test]
    fn build_a_query_golden_bytes() {
        let q = build_a_query("example.com");
        let expected: Vec<u8> = vec![
            0x00, 0x00, // ID
            0x01, 0x00, // RD
            0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, b'e', b'x', b'a', b'm', b'p',
            b'l', b'e', 0x03, b'c', b'o', b'm', 0x00, 0x00, 0x01, 0x00, 0x01,
        ];
        assert_eq!(q, expected);
    }

    #[test]
    fn parse_valid_a_answer() {
        let b = resp_fixture(0x8180, 1, Some([93, 184, 216, 34]));
        let r = parse_response(&b).unwrap();
        assert_eq!(r.rcode, 0);
        assert_eq!(r.answers, vec![IpAddr::from([93, 184, 216, 34])]);
    }

    #[test]
    fn parse_servfail_rcode() {
        let b = resp_fixture(0x8181, 0, None);
        let r = parse_response(&b).unwrap();
        assert_eq!(r.rcode, 1);
        assert_eq!(rcode_name(r.rcode), "SERVFAIL");
        assert!(r.answers.is_empty());
    }

    #[test]
    fn parse_truncated_flag_parses() {
        // TC bit (0x0200) set, rcode NOERROR, no answers.
        let b = resp_fixture(0x8380, 0, None);
        let r = parse_response(&b).unwrap();
        assert_eq!(r.rcode, 0);
        assert!(r.answers.is_empty());
    }

    #[test]
    fn parse_no_answers_noerror() {
        let b = resp_fixture(0x8180, 0, None);
        let r = parse_response(&b).unwrap();
        assert_eq!(r.rcode, 0);
        assert!(r.answers.is_empty());
    }

    #[test]
    fn parse_pointer_name_in_answer() {
        // The fixture uses a 0xC00C compression pointer in the answer name.
        let b = resp_fixture(0x8180, 1, Some([1, 1, 1, 1]));
        let r = parse_response(&b).unwrap();
        assert_eq!(r.answers, vec![IpAddr::from([1, 1, 1, 1])]);
    }

    #[test]
    fn parse_short_input_fails() {
        assert!(parse_response(&[0u8; 5]).is_err());
    }
}
