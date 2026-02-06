use std::net::{Ipv4Addr, SocketAddrV4};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, debug};

use super::copy::bidirectional_copy;

/// Run the inbound proxy: accept IPv6 TCP connections and forward to IPv4 localhost.
///
/// This is the core NAT64 inbound path. Traffic arrives on the pod's IPv6
/// address at `listen_port`, and gets forwarded to the application container
/// on `forward_addr:forward_port` via IPv4 localhost (shared network namespace).
pub async fn run_inbound_proxy(
    listen_port: u16,
    forward_addr: Ipv4Addr,
    forward_port: u16,
) -> anyhow::Result<()> {
    let listen_addr = format!("[::]:{listen_port}");
    let listener = TcpListener::bind(&listen_addr).await?;
    info!(listen_addr, "inbound proxy listening");

    let forward_sockaddr = SocketAddrV4::new(forward_addr, forward_port);

    loop {
        let (ipv6_stream, src_addr) = listener.accept().await?;
        debug!(%src_addr, "accepted inbound IPv6 connection");

        tokio::spawn(async move {
            match handle_inbound(ipv6_stream, forward_sockaddr).await {
                Ok((up, down)) => {
                    debug!(%src_addr, up, down, "inbound connection completed");
                }
                Err(e) => {
                    error!(%src_addr, error = %e, "inbound connection failed");
                }
            }
        });
    }
}

async fn handle_inbound(
    ipv6_stream: TcpStream,
    forward_addr: SocketAddrV4,
) -> anyhow::Result<(u64, u64)> {
    let ipv4_stream = TcpStream::connect(forward_addr).await?;
    debug!(%forward_addr, "connected to IPv4 target");
    let (up, down) = bidirectional_copy(ipv6_stream, ipv4_stream).await?;
    Ok((up, down))
}
