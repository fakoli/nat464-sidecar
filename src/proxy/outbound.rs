use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info, debug};

use crate::happy_eyeballs::HappyEyeballsConfig;
use crate::socks5;

/// Run the outbound SOCKS5 proxy on IPv4 localhost.
///
/// Applications in the pod connect to this proxy via SOCKS5 CONNECT.
/// The proxy resolves the destination hostname and uses Happy Eyeballs v3
/// to race connections across IPv4/IPv6, providing transparent outbound
/// access for IPv4-only applications in an IPv6-only pod network.
pub async fn run_outbound_proxy(
    proxy_port: u16,
    he_config: HappyEyeballsConfig,
) -> anyhow::Result<()> {
    let listen_addr = format!("127.0.0.1:{proxy_port}");
    let listener = TcpListener::bind(&listen_addr).await?;
    info!(listen_addr, "outbound SOCKS5 proxy listening");

    let he_config = Arc::new(he_config);

    loop {
        let (client_stream, src_addr) = listener.accept().await?;
        debug!(%src_addr, "accepted outbound SOCKS5 connection");

        let he_config = Arc::clone(&he_config);
        tokio::spawn(async move {
            if let Err(e) = socks5::handle_client(client_stream, &he_config).await {
                error!(%src_addr, error = %e, "outbound SOCKS5 connection failed");
            }
        });
    }
}
