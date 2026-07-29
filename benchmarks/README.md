# Benchmarks (2026-07-29)

## Environment

- Host: `Linux 7.0.11-1-cachyos x86_64`
- CPU: `11th Gen Intel(R) Core(TM) i7-11800H` (8C/16T)
- Python: `3.13.12`
- Rust: `1.92.0`
- uv: `0.11.31`
- Upstream PTT: `88429bb90acef55673f421f45038878809b1e577`
- Upstream RTN: `bdb9973109eb489be831af0c39bdf9c27e3378ed`
- This repo: `efc9791` + local working tree

## Python API: upstream vs this repo

Source data: [`python_vs_rust_2026-07-29.csv`](python_vs_rust_2026-07-29.csv). Each parser receives a 100-title untimed warm-up before measurement; both modes use the same Python executable.

| Parser | N | Upstream (items/s) | Rust port (items/s) | Speedup | Upstream p50 (ms) | Rust p50 (ms) | Upstream p95 (ms) | Rust p95 (ms) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `PTT.parse_title` | 1,000 | 1,827.7 | 40,325.0 | 22.06x | 0.550 | 0.024 | 0.577 | 0.030 |
| `RTN.parse` | 1,000 | 1,737.6 | 30,311.3 | 17.44x | 0.582 | 0.032 | 0.609 | 0.041 |
| `PTT.parse_title` | 10,000 | 1,830.2 | 41,182.0 | 22.50x | 0.551 | 0.024 | 0.579 | 0.030 |
| `RTN.parse` | 10,000 | 1,727.4 | 31,224.6 | 18.08x | 0.584 | 0.031 | 0.610 | 0.039 |
| `PTT.parse_title` | 30,000 | 1,815.6 | 40,355.6 | 22.23x | 0.556 | 0.024 | 0.582 | 0.031 |
| `RTN.parse` | 30,000 | 1,708.8 | 30,284.9 | 17.72x | 0.590 | 0.032 | 0.618 | 0.041 |

Geometric mean throughput speedup (all rows): **19.88x** (**22.26x** for PTT and **17.75x** for RTN).

## Rust native core

| Benchmark | Mean time | Per-item equivalent |
|---|---:|---:|
| `ptt_core/parse_title_translate_false` | 24.895 us | 24.895 us |
| `ptt_core/parse_title_translate_true` | 24.934 us | 24.934 us |
| `ptt_core/parse_many_128_translate_false` | 0.993 ms / 128 items | 7.76 us |
| `ptt_core/parse_many_128_translate_true` | 0.999 ms / 128 items | 7.80 us |
| `rtn_core/parse` | 25.631 us | 25.631 us |
| `rtn_core/parse_fetch_rank` | 279.52 us | 279.52 us |
| `rtn_core/scalar_128_parse_fetch_rank` | 35.946 ms / 128 items | 280.83 us |
| `rtn_core/batch_128_parse_fetch_rank` | 4.307 ms / 128 items | 33.65 us |

## Commands Used

```bash
uv run scripts/bench_compare.py | tee benchmarks/python_vs_rust_2026-07-29.csv
cargo bench -p ptt-core --bench ptt_bench
cargo bench -p rtn-core --bench rtn_bench
```
