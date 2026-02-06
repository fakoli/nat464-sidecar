use std::net::{IpAddr, SocketAddr};
use tokio::net::TcpStream;
use tracing::{debug, error};

use crate::happy_eyeballs::HappyEyeballsConfig;
use crate::proxy::copy::bidirectional_copy;

use super::handshake::{self, ConnectRequest, DestAddr};

/// Handle a single SOCKS5 client connection end-to-end.
///
/// 1. Perform SOCKS5 handshake to get destination
/// 2. Resolve and connect using Happy Eyeballs v3 (for domains)
///    or directly (for IP addresses)
/// 3. Send SOCKS5 success reply
/// 4. Relay bytes bidirectionally
pub async fn handle_client(
    mut client: TcpStream,
    he_config: &HappyEyeballsConfig,
) -> anyhow::Result<()> {
    let ConnectRequest { dest, port } = handshake::perform_handshake(&mut client).await?;

    let remote = match dest {
        DestAddr::Domain(ref hostname) => {
            // Use Happy Eyeballs v3 for hostname-based connections
            debug!(%hostname, port, "resolving with Happy Eyeballs v3");
            crate::happy_eyeballs::connect(hostname, port, he_config).await?
        }
        DestAddr::Ip4(addr) => {
            let sockaddr = SocketAddr::new(IpAddr::V4(addr), port);
            debug!(%sockaddr, "direct IPv4 connect");
            TcpStream::connect(sockaddr).await?
        }
        DestAddr::Ip6(addr) => {
            let sockaddr = SocketAddr::new(IpAddr::V6(addr), port);
            debug!(%sockaddr, "direct IPv6 connect");
            TcpStream::connect(sockaddr).await?
        }
    };

    // Send SOCKS5 success reply
    handshake::send_reply(&mut client, super::REPLY_SUCCEEDED).await?;
    debug!("SOCKS5 relay established");

    // Relay bytes between client and remote
    match bidirectional_copy(client, remote).await {
        Ok((up, down)) => {
            debug!(up, down, "SOCKS5 relay completed");
        }
        Err(e) => {
            // Connection resets during relay are expected (client disconnect)
            if e.kind() != std::io::ErrorKind::ConnectionReset {
                error!(error = %e, "SOCKS5 relay error");
            }
        }
    }

    Ok(())
}
