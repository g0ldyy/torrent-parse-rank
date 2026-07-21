# Benchmarks (2026-07-22)

## Environment

- Host: `Linux 7.0.11-1-cachyos x86_64`
- CPU: `11th Gen Intel(R) Core(TM) i7-11800H` (8C/16T)
- Python: `3.13.12`
- Rust: `1.92.0`
- uv: `0.11.19`

## Python API: upstream vs this repo

Source data: [`python_vs_rust_2026-07-22.csv`](python_vs_rust_2026-07-22.csv). Each parser receives a 100-title untimed warm-up before measurement; both modes use the same Python executable.

| Parser | N | Upstream (items/s) | Rust port (items/s) | Speedup | Upstream p50 (ms) | Rust p50 (ms) | Upstream p95 (ms) | Rust p95 (ms) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `PTT.parse_title` | 1,000 | 1,863.9 | 12,323.7 | 6.61x | 0.544 | 0.082 | 0.566 | 0.091 |
| `RTN.parse` | 1,000 | 1,745.1 | 10,118.4 | 5.80x | 0.577 | 0.099 | 0.617 | 0.110 |
| `PTT.parse_title` | 10,000 | 1,829.5 | 12,118.0 | 6.62x | 0.550 | 0.082 | 0.585 | 0.093 |
| `RTN.parse` | 10,000 | 1,718.0 | 9,934.2 | 5.78x | 0.585 | 0.101 | 0.622 | 0.112 |
| `PTT.parse_title` | 30,000 | 1,823.9 | 12,171.5 | 6.67x | 0.553 | 0.083 | 0.582 | 0.092 |
| `RTN.parse` | 30,000 | 1,716.0 | 9,872.3 | 5.75x | 0.587 | 0.102 | 0.619 | 0.113 |

Geometric mean throughput speedup (all rows): **6.19x**.

## Rust native core (Criterion mean time)

| Benchmark | Mean time | Per-item equivalent |
|---|---:|---:|
| `ptt_core/parse_title_translate_false` | 79.27 us | 79.27 us |
| `ptt_core/parse_title_translate_true` | 80.08 us | 80.08 us |
| `ptt_core/parse_many_128_translate_false` | 10.271 ms / 128 items | 80.24 us |
| `ptt_core/parse_many_128_translate_true` | 10.300 ms / 128 items | 80.47 us |
| `rtn_core/parse` | 83.34 us | 83.34 us |
| `rtn_core/parse_fetch_rank` | 88.10 us | 88.10 us |
| `rtn_core/batch_128_parse_fetch_rank` | 11.220 ms / 128 items | 87.66 us |

## Commands Used

```bash
uv run scripts/bench_compare.py | tee benchmarks/python_vs_rust_2026-07-22.csv
cargo bench -p ptt-core --bench ptt_bench
cargo bench -p rtn-core --bench rtn_bench
```
