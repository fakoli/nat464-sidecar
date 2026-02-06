use std::net::{IpAddr, SocketAddr};
use tokio::net::lookup_host;
use tracing::debug;

/// Resolved addresses from DNS lookup, separated by address family.
///
/// Happy Eyeballs v3 needs addresses sorted by family for interleaved racing:
/// prefer IPv6 first (since we're in an IPv6-native environment), then IPv4.
#[derive(Debug)]
pub struct ResolvedAddresses {
    pub ipv6: Vec<SocketAddr>,
    pub ipv4: Vec<SocketAddr>,
}

/// Resolve a hostname to both IPv4 and IPv6 addresses.
///
/// Uses tokio's built-in DNS resolution which calls getaddrinfo(3) under the hood.
/// In production K8s, CoreDNS with DNS64 will synthesize AAAA records for IPv4-only
/// destinations, so we'll get IPv6 addresses even for IPv4 hosts.
///
/// Returns addresses separated by family for Happy Eyeballs interleaving.
pub async fn resolve(hostname: &str, port: u16) -> anyhow::Result<ResolvedAddresses> {
    let lookup = format!("{hostname}:{port}");
    let addrs: Vec<SocketAddr> = lookup_host(&lookup).await?.collect();

    if addrs.is_empty() {
        anyhow::bail!("DNS resolution returned no addresses for {hostname}");
    }

    let mut ipv6 = Vec::new();
    let mut ipv4 = Vec::new();

    for addr in addrs {
        match addr.ip() {
            IpAddr::V6(_) => ipv6.push(addr),
            IpAddr::V4(_) => ipv4.push(addr),
        }
    }

    debug!(
        hostname,
        ipv6_count = ipv6.len(),
        ipv4_count = ipv4.len(),
        "DNS resolution complete"
    );

    Ok(ResolvedAddresses { ipv6, ipv4 })
}

/// Interleave IPv6 and IPv4 addresses for Happy Eyeballs racing.
///
/// Per RFC 8305 Section 4: "The client SHOULD alternate between
/// addresses from the two address families." IPv6 addresses come first
/// since we're in an IPv6-native environment.
pub fn interleave(resolved: &ResolvedAddresses) -> Vec<SocketAddr> {
    let mut result = Vec::with_capacity(resolved.ipv6.len() + resolved.ipv4.len());
    let mut v6_iter = resolved.ipv6.iter();
    let mut v4_iter = resolved.ipv4.iter();

    loop {
        let v6 = v6_iter.next();
        let v4 = v4_iter.next();

        match (v6, v4) {
            (Some(a6), Some(a4)) => {
                result.push(*a6);
                result.push(*a4);
            }
            (Some(a6), None) => {
                result.push(*a6);
            }
            (None, Some(a4)) => {
                result.push(*a4);
            }
            (None, None) => break,
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6};

    #[test]
    fn test_interleave_balanced() {
        let resolved = ResolvedAddresses {
            ipv6: vec![
                SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 80, 0, 0)),
            ],
            ipv4: vec![
                SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 80)),
            ],
        };
        let result = interleave(&resolved);
        assert_eq!(result.len(), 2);
        assert!(result[0].is_ipv6());
        assert!(result[1].is_ipv4());
    }

    #[test]
    fn test_interleave_ipv6_only() {
        let resolved = ResolvedAddresses {
            ipv6: vec![
                SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 80, 0, 0)),
                SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 80, 0, 0)),
            ],
            ipv4: vec![],
        };
        let result = interleave(&resolved);
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|a| a.is_ipv6()));
    }

    #[test]
    fn test_interleave_ipv4_only() {
        let resolved = ResolvedAddresses {
            ipv6: vec![],
            ipv4: vec![
                SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 80)),
            ],
        };
        let result = interleave(&resolved);
        assert_eq!(result.len(), 1);
        assert!(result[0].is_ipv4());
    }

    #[test]
    fn test_interleave_unbalanced() {
        let resolved = ResolvedAddresses {
            ipv6: vec![
                SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1), 80, 0, 0)),
                SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 2), 80, 0, 0)),
                SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 3), 80, 0, 0)),
            ],
            ipv4: vec![
                SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 1), 80)),
            ],
        };
        let result = interleave(&resolved);
        // Should be: v6[0], v4[0], v6[1], v6[2]
        assert_eq!(result.len(), 4);
        assert!(result[0].is_ipv6());
        assert!(result[1].is_ipv4());
        assert!(result[2].is_ipv6());
        assert!(result[3].is_ipv6());
    }
}
