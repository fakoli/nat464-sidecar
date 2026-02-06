use clap::Parser;
use std::net::Ipv4Addr;

/// nat464-sidecar: NAT464 translation sidecar for IPv6-only Kubernetes clusters.
///
/// Provides bidirectional IPv6 ↔ IPv4 translation within a Kubernetes pod:
/// - Inbound: Accepts IPv6 TCP on the pod IP and forwards to IPv4 localhost
/// - Outbound: SOCKS5 proxy on IPv4 localhost with Happy Eyeballs v3 connection racing
#[derive(Parser, Debug, Clone)]
#[command(name = "nat464-sidecar", version, about)]
pub struct Config {
    /// Port to listen on for inbound IPv6 TCP connections (on [::])
    #[arg(long, default_value_t = 8080)]
    pub listen_port: u16,

    /// Port to forward inbound connections to on the IPv4 localhost
    #[arg(long, default_value_t = 80)]
    pub forward_port: u16,

    /// IPv4 address to forward inbound connections to
    #[arg(long, default_value_t = Ipv4Addr::LOCALHOST)]
    pub forward_addr: Ipv4Addr,

    /// Port for the outbound SOCKS5 proxy (on 127.0.0.1)
    #[arg(long, default_value_t = 1080)]
    pub proxy_port: u16,

    /// Port for the health check HTTP endpoint
    #[arg(long, default_value_t = 9464)]
    pub health_port: u16,

    /// Connection Attempt Delay for Happy Eyeballs v3 (milliseconds)
    #[arg(long, default_value_t = 250)]
    pub he_connection_delay_ms: u64,

    /// Resolution Delay for Happy Eyeballs v3 (milliseconds)
    #[arg(long, default_value_t = 50)]
    pub he_resolution_delay_ms: u64,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    pub log_level: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::parse_from(["nat464-sidecar"]);
        assert_eq!(config.listen_port, 8080);
        assert_eq!(config.forward_port, 80);
        assert_eq!(config.forward_addr, Ipv4Addr::LOCALHOST);
        assert_eq!(config.proxy_port, 1080);
        assert_eq!(config.health_port, 9464);
        assert_eq!(config.he_connection_delay_ms, 250);
        assert_eq!(config.he_resolution_delay_ms, 50);
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn test_custom_config() {
        let config = Config::parse_from([
            "nat464-sidecar",
            "--listen-port", "9090",
            "--forward-port", "3000",
            "--forward-addr", "127.0.0.2",
            "--proxy-port", "1081",
            "--health-port", "9465",
            "--he-connection-delay-ms", "300",
            "--he-resolution-delay-ms", "100",
            "--log-level", "debug",
        ]);
        assert_eq!(config.listen_port, 9090);
        assert_eq!(config.forward_port, 3000);
        assert_eq!(config.forward_addr, Ipv4Addr::new(127, 0, 0, 2));
        assert_eq!(config.proxy_port, 1081);
        assert_eq!(config.health_port, 9465);
        assert_eq!(config.he_connection_delay_ms, 300);
        assert_eq!(config.he_resolution_delay_ms, 100);
        assert_eq!(config.log_level, "debug");
    }
}
