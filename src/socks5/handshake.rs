use std::net::{Ipv4Addr, Ipv6Addr};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::debug;

use super::*;

/// Destination address parsed from a SOCKS5 CONNECT request.
#[derive(Debug)]
pub enum DestAddr {
    /// IPv4 address
    Ip4(Ipv4Addr),
    /// IPv6 address
    Ip6(Ipv6Addr),
    /// Domain name (preserved for Happy Eyeballs hostname-based racing)
    Domain(String),
}

/// Result of a successful SOCKS5 handshake: the destination address and port.
#[derive(Debug)]
pub struct ConnectRequest {
    pub dest: DestAddr,
    pub port: u16,
}

/// Perform the SOCKS5 handshake on a client connection.
///
/// Implements the RFC 1928 handshake:
/// 1. Client sends version + auth method list
/// 2. Server selects NO_AUTH (0x00) — safe because we only listen on localhost
/// 3. Client sends CONNECT request with destination address
/// 4. We parse and return the destination; caller establishes the remote connection
///    and sends the reply.
pub async fn perform_handshake(stream: &mut TcpStream) -> anyhow::Result<ConnectRequest> {
    // --- Method selection ---
    let version = stream.read_u8().await?;
    if version != SOCKS_VERSION {
        anyhow::bail!("unsupported SOCKS version: {version}");
    }

    let nmethods = stream.read_u8().await?;
    let mut methods = vec![0u8; nmethods as usize];
    stream.read_exact(&mut methods).await?;

    if !methods.contains(&AUTH_NO_AUTH) {
        // We only support no-auth (localhost-only proxy)
        stream.write_all(&[SOCKS_VERSION, 0xFF]).await?;
        anyhow::bail!("client does not support NO_AUTH method");
    }

    // Accept NO_AUTH
    stream.write_all(&[SOCKS_VERSION, AUTH_NO_AUTH]).await?;
    debug!("SOCKS5 auth negotiated: NO_AUTH");

    // --- CONNECT request ---
    let ver = stream.read_u8().await?;
    if ver != SOCKS_VERSION {
        anyhow::bail!("unexpected SOCKS version in request: {ver}");
    }

    let cmd = stream.read_u8().await?;
    if cmd != CMD_CONNECT {
        // Send command-not-supported reply
        send_reply(stream, REPLY_COMMAND_NOT_SUPPORTED).await?;
        anyhow::bail!("unsupported SOCKS5 command: {cmd}");
    }

    let _rsv = stream.read_u8().await?; // reserved byte

    let atyp = stream.read_u8().await?;
    let dest = match atyp {
        ATYP_IPV4 => {
            let mut octets = [0u8; 4];
            stream.read_exact(&mut octets).await?;
            DestAddr::Ip4(Ipv4Addr::from(octets))
        }
        ATYP_DOMAIN => {
            let len = stream.read_u8().await? as usize;
            let mut domain_buf = vec![0u8; len];
            stream.read_exact(&mut domain_buf).await?;
            let domain = String::from_utf8(domain_buf)?;
            DestAddr::Domain(domain)
        }
        ATYP_IPV6 => {
            let mut octets = [0u8; 16];
            stream.read_exact(&mut octets).await?;
            DestAddr::Ip6(Ipv6Addr::from(octets))
        }
        _ => {
            send_reply(stream, REPLY_ADDRESS_TYPE_NOT_SUPPORTED).await?;
            anyhow::bail!("unsupported SOCKS5 address type: {atyp}");
        }
    };

    let port = stream.read_u16().await?;
    debug!(?dest, port, "SOCKS5 CONNECT request parsed");

    Ok(ConnectRequest { dest, port })
}

/// Send a SOCKS5 reply with the given status code.
///
/// Uses 0.0.0.0:0 as the bound address (we don't expose real bind info).
pub async fn send_reply(stream: &mut TcpStream, reply: u8) -> anyhow::Result<()> {
    let response = [
        SOCKS_VERSION,
        reply,
        0x00,       // reserved
        ATYP_IPV4,  // address type: IPv4
        0, 0, 0, 0, // bind addr: 0.0.0.0
        0, 0,       // bind port: 0
    ];
    stream.write_all(&response).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    /// Helper: connect a client and server for testing the handshake.
    async fn test_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let client = TcpStream::connect(addr).await.unwrap();
        let (server, _) = listener.accept().await.unwrap();
        (client, server)
    }

    #[tokio::test]
    async fn test_handshake_domain() {
        let (mut client, mut server) = test_pair().await;

        // Simulate SOCKS5 client in a separate task
        let client_task = tokio::spawn(async move {
            // Method negotiation: version=5, 1 method, NO_AUTH
            client.write_all(&[0x05, 0x01, 0x00]).await.unwrap();

            // Read server's method selection reply
            let mut reply = [0u8; 2];
            tokio::io::AsyncReadExt::read_exact(&mut client, &mut reply).await.unwrap();
            assert_eq!(reply, [0x05, 0x00]); // version=5, NO_AUTH selected

            // CONNECT request: domain "example.com" port 80
            let domain = b"example.com";
            let mut request = vec![
                0x05, // version
                0x01, // CMD_CONNECT
                0x00, // reserved
                0x03, // ATYP_DOMAIN
                domain.len() as u8,
            ];
            request.extend_from_slice(domain);
            request.extend_from_slice(&80u16.to_be_bytes());
            client.write_all(&request).await.unwrap();

            client
        });

        let result = perform_handshake(&mut server).await.unwrap();
        match result.dest {
            DestAddr::Domain(ref d) => assert_eq!(d, "example.com"),
            _ => panic!("expected Domain, got {:?}", result.dest),
        }
        assert_eq!(result.port, 80);

        client_task.await.unwrap();
    }

    #[tokio::test]
    async fn test_handshake_ipv4() {
        let (mut client, mut server) = test_pair().await;

        let client_task = tokio::spawn(async move {
            // Method negotiation
            client.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
            let mut reply = [0u8; 2];
            tokio::io::AsyncReadExt::read_exact(&mut client, &mut reply).await.unwrap();

            // CONNECT to 10.0.0.1:443
            let request = [
                0x05, 0x01, 0x00, // ver, cmd, rsv
                0x01,             // ATYP_IPV4
                10, 0, 0, 1,     // 10.0.0.1
                0x01, 0xBB,      // port 443
            ];
            client.write_all(&request).await.unwrap();
            client
        });

        let result = perform_handshake(&mut server).await.unwrap();
        match result.dest {
            DestAddr::Ip4(addr) => assert_eq!(addr, Ipv4Addr::new(10, 0, 0, 1)),
            _ => panic!("expected Ip4, got {:?}", result.dest),
        }
        assert_eq!(result.port, 443);

        client_task.await.unwrap();
    }
}
