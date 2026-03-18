# Benchmarks (2026-03-18)

## Environment

- Host: `Linux cachyos 6.19.7-1-cachyos x86_64`
- CPU: `11th Gen Intel(R) Core(TM) i7-11800H` (8C/16T)
- Python: `3.14.3`
- Rust: `1.92.0`
- uv: `0.10.10`

## Python API: upstream vs this repo

Source data: [`python_vs_rust_2026-03-18.csv`](python_vs_rust_2026-03-18.csv)

| Parser | N | Upstream (items/s) | Rust port (items/s) | Speedup | Upstream p50 (ms) | Rust p50 (ms) | Upstream p95 (ms) | Rust p95 (ms) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `PTT.parse_title` | 1,000 | 1,822.0 | 5,596.3 | 3.07x | 0.552 | 0.172 | 0.579 | 0.208 |
| `RTN.parse` | 1,000 | 1,720.8 | 5,434.3 | 3.16x | 0.585 | 0.183 | 0.613 | 0.221 |
| `PTT.parse_title` | 10,000 | 1,708.0 | 5,702.4 | 3.34x | 0.586 | 0.175 | 0.637 | 0.211 |
| `RTN.parse` | 10,000 | 1,617.1 | 5,365.0 | 3.32x | 0.617 | 0.185 | 0.681 | 0.223 |
| `PTT.parse_title` | 30,000 | 1,832.2 | 5,763.9 | 3.15x | 0.550 | 0.173 | 0.575 | 0.208 |
| `RTN.parse` | 30,000 | 1,726.1 | 5,446.9 | 3.16x | 0.583 | 0.183 | 0.610 | 0.220 |

Geometric mean throughput speedup (all rows): **3.20x**.

## Rust native core (Criterion mean time)

| Benchmark | Mean time | Per-item equivalent |
|---|---:|---:|
| `ptt_core/parse_title_translate_false` | 156.47 us | 156.47 us |
| `ptt_core/parse_title_translate_true` | 159.81 us | 159.81 us |
| `ptt_core/parse_many_128_translate_false` | 20.064 ms / 128 items | 156.75 us |
| `ptt_core/parse_many_128_translate_true` | 19.845 ms / 128 items | 155.04 us |
| `rtn_core/parse` | 156.05 us | 156.05 us |
| `rtn_core/parse_fetch_rank` | 164.88 us | 164.88 us |
| `rtn_core/batch_128_parse_fetch_rank` | 21.281 ms / 128 items | 166.25 us |

## Commands Used

```bash
uv run scripts/bench_compare.py | tee benchmarks/python_vs_rust_2026-03-18.csv
cargo bench -p ptt-core --bench ptt_bench
cargo bench -p rtn-core --bench rtn_bench
```
