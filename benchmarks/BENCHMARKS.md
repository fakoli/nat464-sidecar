# Benchmark History

Track performance across sessions to detect regressions or improvements.

## How to Add a New Entry

After running `benchmark.sh`, create a JSON file:

```
benchmarks/YYYY-MM-DD_short-description.json
```

Then append a row to the summary table below.

## Summary

| Date | Label | Latency p50 (baseline) | Latency p50 (sidecar) | Overhead (ms) | Overhead (%) | Throughput Reduction (%) | Validation |
|------|-------|------------------------|------------------------|---------------|--------------|--------------------------|------------|
| 2026-02-06 | M1 PoC Validation | 0.18ms | 0.26ms | +0.08ms | 44% | ~19-29% | 5/5 PASS |

## Notes

- **Latency overhead** is dominated by per-connection cost (extra TCP hop through sidecar). On larger payloads or persistent connections, the overhead percentage shrinks.
- **Throughput** is measured on small nginx default pages (~615 bytes). Real workloads with larger payloads will show lower overhead.
- **Outbound SOCKS5** latency to external hosts is dominated by internet RTT, not sidecar overhead.
- All measurements from inside a Multipass VM (2 CPU, 4G RAM) running k3s.
