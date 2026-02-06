use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio::time::sleep;
use tracing::debug;

use super::resolver;
use super::HappyEyeballsConfig;

/// Connect to a hostname using Happy Eyeballs v3 connection racing.
///
/// Implements the core algorithm from RFC 8305 (HEv2) with v3 extensions:
///
/// 1. Resolve hostname to A + AAAA records concurrently
/// 2. Interleave addresses: IPv6 first, alternating families
/// 3. Start connection attempts staggered by Connection Attempt Delay (250ms)
/// 4. First successful TCP handshake wins; cancel remaining attempts
///
/// This ensures fast connections even when one address family is broken
/// or slow, while still preferring IPv6 in IPv6-native environments.
pub async fn connect(
    hostname: &str,
    port: u16,
    config: &HappyEyeballsConfig,
) -> anyhow::Result<TcpStream> {
    let resolved = resolver::resolve(hostname, port).await?;
    let addrs = resolver::interleave(&resolved);

    if addrs.is_empty() {
        anyhow::bail!("no addresses resolved for {hostname}:{port}");
    }

    debug!(
        hostname,
        port,
        addr_count = addrs.len(),
        "starting Happy Eyeballs v3 racing"
    );

    // If only one address, just connect directly — no racing needed
    if addrs.len() == 1 {
        let addr = addrs[0];
        debug!(%addr, "single address, connecting directly");
        let stream = TcpStream::connect(addr).await?;
        return Ok(stream);
    }

    // Race connections using tokio::select! with staggered starts
    race_connections(&addrs, config).await
}

/// Race connection attempts with staggered starts.
///
/// We spawn each connection attempt as a separate task with a delay,
/// and use a channel to collect the first success. This avoids the
/// complexity of managing a dynamic select! and correctly handles
/// cleanup of losing connections.
async fn race_connections(
    addrs: &[SocketAddr],
    config: &HappyEyeballsConfig,
) -> anyhow::Result<TcpStream> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<(TcpStream, SocketAddr, usize)>(1);
    let cancel = tokio_util::sync::CancellationToken::new();

    for (i, &addr) in addrs.iter().enumerate() {
        let tx = tx.clone();
        let delay = config.connection_attempt_delay * i as u32;
        let cancel = cancel.clone();

        tokio::spawn(async move {
            // Wait for our turn (staggered start)
            if !delay.is_zero() {
                tokio::select! {
                    _ = sleep(delay) => {}
                    _ = cancel.cancelled() => return,
                }
            }

            // Check if another attempt already won
            if cancel.is_cancelled() {
                return;
            }

            debug!(%addr, attempt = i, "attempting connection");

            // Race the connection attempt against cancellation
            tokio::select! {
                result = TcpStream::connect(addr) => {
                    match result {
                        Ok(stream) => {
                            // We won the race! Send the stream back.
                            let _ = tx.send((stream, addr, i)).await;
                        }
                        Err(e) => {
                            debug!(%addr, attempt = i, error = %e, "connection attempt failed");
                        }
                    }
                }
                _ = cancel.cancelled() => {
                    debug!(%addr, attempt = i, "connection attempt cancelled (another won)");
                }
            }
        });
    }

    // Drop our sender so the channel closes when all tasks complete
    drop(tx);

    // Wait for the first successful connection
    match rx.recv().await {
        Some((stream, addr, attempt)) => {
            let family = if addr.is_ipv6() { "IPv6" } else { "IPv4" };
            debug!(%addr, attempt, family, "Happy Eyeballs winner");

            // Cancel remaining attempts
            cancel.cancel();

            Ok(stream)
        }
        None => {
            // All attempts failed
            anyhow::bail!("all connection attempts failed")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddrV4};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_race_single_address() {
        // Start a local TCP server
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let config = HappyEyeballsConfig::new(250, 50);

        let connect_task = tokio::spawn(async move {
            race_connections(&[addr], &config).await
        });

        // Accept the connection
        let (_server_stream, _) = listener.accept().await.unwrap();

        let result = connect_task.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_race_multiple_addresses() {
        // Start two local TCP servers
        let listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr1 = listener1.local_addr().unwrap();
        let listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr2 = listener2.local_addr().unwrap();

        let config = HappyEyeballsConfig::new(250, 50);

        let addrs = vec![addr1, addr2];
        let connect_task = tokio::spawn(async move {
            race_connections(&addrs, &config).await
        });

        // Accept on both (one will win the race)
        let accept1 = tokio::spawn(async move {
            listener1.accept().await
        });
        let accept2 = tokio::spawn(async move {
            listener2.accept().await
        });

        let result = connect_task.await.unwrap();
        assert!(result.is_ok());

        // Clean up
        drop(accept1);
        drop(accept2);
    }

    #[tokio::test]
    async fn test_race_first_fails_second_succeeds() {
        // First address: unreachable (connect will fail)
        let bad_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 1), 1));

        // Second address: local server (will succeed)
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let good_addr = listener.local_addr().unwrap();

        // Use very short delay so test runs fast
        let config = HappyEyeballsConfig::new(50, 10);

        let addrs = vec![bad_addr, good_addr];
        let connect_task = tokio::spawn(async move {
            race_connections(&addrs, &config).await
        });

        // Accept on the good listener
        let (_server_stream, _) = listener.accept().await.unwrap();

        let result = connect_task.await.unwrap();
        assert!(result.is_ok());
    }
}
