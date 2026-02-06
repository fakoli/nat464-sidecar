# Changelog

All notable changes to nat464-sidecar are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versioning: milestones (M1, M2, M3, M4).

## [Unreleased]

## [M1] - 2026-02-06

### Milestone 1: TCP PoC

#### Core
- Inbound proxy: IPv6 `[::]:listen_port` -> IPv4 `127.0.0.1:forward_port` via `tokio::io::copy_bidirectional`
- Outbound SOCKS5 proxy: RFC 1928 handshake, domain-preserving CONNECT, Happy Eyeballs v3 connection racing
- Health server: `/healthz` and `/readyz` endpoints via hyper
- NAT64 prefix synthesis: `64:ff9b::/96` (RFC 6052)
- Address interleaving: IPv6-first alternation per RFC 8305 Section 4
- CLI config via clap derive mode
- 17 unit tests passing

#### Deploy
- `deploy/example-pod.yaml` - Dual-container pod (IPv4-only nginx + sidecar)
- `deploy/ipv6-peer-nginx.yaml` - IPv6-only nginx peer pod for outbound testing
- `deploy/benchmark-pod.yaml` - iperf3 pod for manual TCP throughput testing
- Dockerfile with multi-stage build (release, LTO+strip, ~1.4MB binary)

#### Testing & Validation
- `k8s-sidecar-testing/scripts/vm-setup.sh` - Multipass VM provisioning
- `k8s-sidecar-testing/scripts/k3s-setup.sh` - Dual-stack k3s with CoreDNS DNS64
- `k8s-sidecar-testing/scripts/build-image.sh` - Container build + k3s import
- `k8s-sidecar-testing/scripts/deploy-test.sh` - 8 automated tests (health, IPv4-only proof, direct IPv6 refusal, inbound translation, outbound external, outbound peer pod, logs, status)
- `k8s-sidecar-testing/scripts/validate-paths.sh` - Focused 5-test path validation (PASS/FAIL)
- `k8s-sidecar-testing/scripts/benchmark.sh` - HTTP latency/throughput benchmarks (p50/p95/p99, baseline vs sidecar)
- `k8s-sidecar-testing/scripts/teardown.sh` - Cleanup script

#### Validated
- nginx confirmed IPv4-only (`listen 80;` ConfigMap, `0.0.0.0:80` only in `ss`)
- Direct IPv6 to nginx:80 refused (proves sidecar is required)
- Inbound IPv6->IPv4 translation via sidecar port 8080
- Outbound SOCKS5 to IPv6 peer pod inside cluster
- Outbound SOCKS5 to external host (example.com)

#### Benchmark Baseline (M1)
- Inbound latency overhead: +0.08ms at p50 (0.18ms -> 0.26ms)
- Inbound throughput reduction: ~19-29% on small responses
- See `benchmarks/2026-02-06_m1-poc-validation.json`
