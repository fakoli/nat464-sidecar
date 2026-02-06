mod resolver;
mod racer;

pub use racer::connect;

use std::time::Duration;

/// Configuration for Happy Eyeballs v3 connection racing.
///
/// Based on RFC 8305 (Happy Eyeballs v2) with v3 draft extensions:
/// - Staggered connection attempts across address families
/// - Resolution delay for DNS response interleaving
/// - Address synthesis via Well-Known Prefix 64:ff9b::/96 (RFC 6052)
#[derive(Debug, Clone)]
pub struct HappyEyeballsConfig {
    /// Connection Attempt Delay: time between starting connection attempts
    /// to different addresses. Default 250ms per RFC 8305 Section 5.
    pub connection_attempt_delay: Duration,

    /// Resolution Delay: after receiving the first DNS response,
    /// wait this long for the other address family's response
    /// before starting connection attempts. Default 50ms.
    pub resolution_delay: Duration,
}

impl HappyEyeballsConfig {
    pub fn new(connection_delay_ms: u64, resolution_delay_ms: u64) -> Self {
        Self {
            connection_attempt_delay: Duration::from_millis(connection_delay_ms),
            resolution_delay: Duration::from_millis(resolution_delay_ms),
        }
    }
}

/// The Well-Known Prefix for NAT64 address synthesis (RFC 6052).
/// IPv4 addresses are embedded in the last 32 bits of this /96 prefix.
pub const NAT64_WKP: [u16; 6] = [0x0064, 0xff9b, 0, 0, 0, 0];

/// Synthesize an IPv6 address from an IPv4 address using the Well-Known Prefix.
///
/// Per RFC 6052, the IPv4 address is embedded in bits 96-127 of the IPv6 address:
/// `64:ff9b::a.b.c.d`
pub fn synthesize_ipv6(ipv4: std::net::Ipv4Addr) -> std::net::Ipv6Addr {
    let octets = ipv4.octets();
    std::net::Ipv6Addr::new(
        NAT64_WKP[0],
        NAT64_WKP[1],
        NAT64_WKP[2],
        NAT64_WKP[3],
        NAT64_WKP[4],
        NAT64_WKP[5],
        u16::from_be_bytes([octets[0], octets[1]]),
        u16::from_be_bytes([octets[2], octets[3]]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn test_synthesize_ipv6() {
        // 192.0.2.1 → 64:ff9b::192.0.2.1 = 64:ff9b::c000:201
        let v4 = Ipv4Addr::new(192, 0, 2, 1);
        let v6 = synthesize_ipv6(v4);
        assert_eq!(v6, Ipv6Addr::new(0x64, 0xff9b, 0, 0, 0, 0, 0xc000, 0x0201));
    }

    #[test]
    fn test_synthesize_ipv6_localhost() {
        let v4 = Ipv4Addr::new(127, 0, 0, 1);
        let v6 = synthesize_ipv6(v4);
        assert_eq!(v6, Ipv6Addr::new(0x64, 0xff9b, 0, 0, 0, 0, 0x7f00, 0x0001));
    }

    #[test]
    fn test_synthesize_ipv6_zeros() {
        let v4 = Ipv4Addr::new(0, 0, 0, 0);
        let v6 = synthesize_ipv6(v4);
        assert_eq!(v6, Ipv6Addr::new(0x64, 0xff9b, 0, 0, 0, 0, 0, 0));
    }

    #[test]
    fn test_synthesize_ipv6_max() {
        let v4 = Ipv4Addr::new(255, 255, 255, 255);
        let v6 = synthesize_ipv6(v4);
        assert_eq!(v6, Ipv6Addr::new(0x64, 0xff9b, 0, 0, 0, 0, 0xffff, 0xffff));
    }

    #[test]
    fn test_config_defaults() {
        let config = HappyEyeballsConfig::new(250, 50);
        assert_eq!(config.connection_attempt_delay, Duration::from_millis(250));
        assert_eq!(config.resolution_delay, Duration::from_millis(50));
    }
}
