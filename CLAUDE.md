# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
cargo build                    # Debug build
cargo build --release          # Release build (LTO + strip, ~1.4MB binary)
cargo test                     # Run all 17 unit tests
cargo test <test_name>         # Run a single test, e.g. cargo test test_handshake_domain
cargo test happy_eyeballs      # Run tests in a specific module
cargo clippy -- -W clippy::all # Lint (expect dead_code warnings for Milestone 2 items)
docker build -t nat464-sidecar:latest .  # Container image
```

## Running Locally

```bash
./target/release/nat464-sidecar \
  --listen-port 8080 \
  --forward-port 80 \
  --proxy-port 1080 \
  --health-port 9464 \
  --log-level debug
```

The binary starts three concurrent servers via `tokio::try_join!` — if any fails, the process exits.

## Architecture

nat464-sidecar is a userspace NAT464 translation sidecar for IPv6-only Kubernetes pods. It runs alongside an IPv4-only application container in the same pod (shared network namespace) and provides two translation paths:

### Two Data Paths

**Inbound (IPv6 → IPv4):** External IPv6 traffic → `[::]:listen_port` → sidecar accepts → connects to `127.0.0.1:forward_port` → app container. This is a simple TCP proxy in `proxy/inbound.rs`.

**Outbound (IPv4 → IPv6):** App connects to `127.0.0.1:proxy_port` via SOCKS5 → sidecar performs SOCKS5 handshake → extracts destination hostname → resolves DNS → races connections with Happy Eyeballs v3 → first successful connection wins → bidirectional relay. The call chain is: `proxy/outbound.rs` → `socks5/relay.rs` → `socks5/handshake.rs` + `happy_eyeballs/racer.rs`.

### Module Dependency Flow

```
main.rs → Config (clap)
        → proxy/inbound  → proxy/copy (bidirectional_copy)
        → proxy/outbound → socks5/relay → socks5/handshake (RFC 1928 parsing)
                                        → happy_eyeballs/racer → happy_eyeballs/resolver (DNS)
        → health (hyper HTTP server)
```

### Key Design Decisions

- **Opaque byte streams**: All traffic is proxied as raw bytes via `tokio::io::copy_bidirectional`. No TLS inspection.
- **SOCKS5 preserves hostnames**: Domain-type SOCKS5 CONNECT requests pass the hostname to Happy Eyeballs for proper connection racing (not pre-resolved IPs).
- **Happy Eyeballs v3 racing**: Uses `tokio_util::sync::CancellationToken` + `mpsc` channel pattern. Each connection attempt is a spawned task with staggered delay (`connection_attempt_delay * attempt_index`). First success sends through channel; losers get cancelled.
- **Address interleaving**: `resolver::interleave()` alternates IPv6/IPv4 addresses with IPv6 first (RFC 8305 Section 4).
- **NAT64 prefix**: `64:ff9b::/96` (RFC 6052 Well-Known Prefix) for IPv4→IPv6 address synthesis in `happy_eyeballs/mod.rs::synthesize_ipv6()`.
- **No capabilities required**: Pure userspace — no `NET_ADMIN`, `SYS_ADMIN`, or host networking.

### Error Handling

- `anyhow::Result` throughout for ergonomic error propagation.
- Individual connection errors are logged and don't crash the server (errors handled inside `tokio::spawn`).
- `ConnectionReset` during relay is silently ignored (expected client disconnects).

## Conventions

- Async runtime: Tokio with `#[tokio::main]` and `full` features.
- CLI: clap derive mode. All config is via `--long-flag` CLI args (see `config.rs`).
- Logging: `tracing` crate with structured fields. Use `debug!` for per-connection events, `info!` for server lifecycle, `error!` for failures.
- Tests: In-module `#[cfg(test)]` blocks. Network tests bind to port 0 for random port allocation. SOCKS5 tests use `test_pair()` helper to create connected TCP stream pairs.

## Milestones

Currently at **Milestone 1 (PoC)** — TCP-only translation. Planned:
- M2: UDP/ICMP, Prometheus metrics, graceful shutdown, `io_uring`
- M3: Mutating webhook auto-injection, Helm chart
- M4: Optional eBPF data plane
