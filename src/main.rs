mod config;
mod happy_eyeballs;
mod health;
mod proxy;
mod socks5;

use clap::Parser;
use tracing::info;

use config::Config;
use happy_eyeballs::HappyEyeballsConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::parse();

    // Initialize structured logging with tracing
    let env_filter = tracing_subscriber::EnvFilter::try_new(&config.log_level)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .init();

    info!(
        listen_port = config.listen_port,
        forward_port = config.forward_port,
        forward_addr = %config.forward_addr,
        proxy_port = config.proxy_port,
        health_port = config.health_port,
        "nat464-sidecar starting"
    );

    let he_config = HappyEyeballsConfig::new(
        config.he_connection_delay_ms,
        config.he_resolution_delay_ms,
    );

    // Spawn all servers concurrently. If any exits with an error,
    // the process terminates (fail-fast for the PoC).
    tokio::try_join!(
        // Inbound: IPv6 → IPv4 translation
        proxy::inbound::run_inbound_proxy(
            config.listen_port,
            config.forward_addr,
            config.forward_port,
        ),
        // Outbound: SOCKS5 proxy with Happy Eyeballs v3
        proxy::outbound::run_outbound_proxy(config.proxy_port, he_config),
        // Health check HTTP server
        health::run_health_server(config.health_port),
    )?;

    Ok(())
}
