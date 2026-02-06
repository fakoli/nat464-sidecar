# nat464-sidecar

A Rust sidecar container that transparently bridges IPv6 and IPv4 within a Kubernetes pod, enabling legacy IPv4 workloads to operate in IPv6-only clusters with zero application changes.

## Architecture

```
                        Pod Network (IPv6)
                           │        ▲
                  inbound  ▼        │  outbound
┌──────────────────────────────────────────────────────┐
│  Kubernetes Pod (shared network namespace)            │
│                                                       │
│  ┌──────────────────────────┐  ┌───────────────────┐ │
│  │  nat464-sidecar          │  │  App Container    │ │
│  │                          │  │  (IPv4-only)      │ │
│  │  INBOUND:                │  │                   │ │
│  │  [::]:PORT (IPv6)  ──────│──│→ 127.0.0.1:PORT  │ │
│  │                          │  │                   │ │
│  │  OUTBOUND:               │  │                   │ │
│  │  127.0.0.1:PROXY ◄──────│──│  (SOCKS5 client)  │ │
│  │  Happy Eyeballs v3  ─────│──│→ IPv6 destination │ │
│  │                          │  │                   │ │
│  └──────────────────────────┘  └───────────────────┘ │
└──────────────────────────────────────────────────────┘
```

**Inbound** (IPv6 network → IPv4 app): IPv6 traffic arrives at the pod IP, the sidecar accepts it and forwards to `127.0.0.1` via IPv4.

**Outbound** (IPv4 app → IPv6 network): The app connects through the sidecar's SOCKS5 proxy on localhost. The sidecar uses Happy Eyeballs v3 to race connections across address families.

## Quick Start

### Build

```bash
cargo build --release
```

### Run locally

```bash
# Start the sidecar (listens on [::]:8080, forwards to 127.0.0.1:80)
./target/release/nat464-sidecar \
  --listen-port 8080 \
  --forward-port 80 \
  --proxy-port 1080 \
  --health-port 9464 \
  --log-level info
```

### Container image

```bash
docker build -t nat464-sidecar:latest .
```

### Kubernetes deployment

```bash
kubectl apply -f deploy/example-pod.yaml
```

## Configuration

| Flag | Default | Description |
|------|---------|-------------|
| `--listen-port` | 8080 | Inbound IPv6 TCP listen port |
| `--forward-port` | 80 | IPv4 localhost forward port |
| `--forward-addr` | 127.0.0.1 | IPv4 forward address |
| `--proxy-port` | 1080 | Outbound SOCKS5 proxy port |
| `--health-port` | 9464 | Health check HTTP port |
| `--he-connection-delay-ms` | 250 | Happy Eyeballs connection attempt delay |
| `--he-resolution-delay-ms` | 50 | Happy Eyeballs DNS resolution delay |
| `--log-level` | info | Log level (trace/debug/info/warn/error) |

## Happy Eyeballs v3

The outbound SOCKS5 proxy implements Happy Eyeballs v3 connection racing:

1. App sends hostname via SOCKS5 CONNECT
2. Sidecar resolves hostname → A + AAAA records
3. Addresses interleaved: IPv6 first, alternating families
4. Connection attempts staggered by 250ms (configurable)
5. First successful TCP handshake wins

Address synthesis uses the Well-Known Prefix `64:ff9b::/96` (RFC 6052).

## Security

- Runs as non-root user
- No `NET_ADMIN` or `SYS_ADMIN` capabilities required
- Read-only root filesystem
- Minimal distroless container image
- All traffic treated as opaque byte streams (no TLS inspection)

## License

Apache-2.0
