# Benchmarks (2026-07-22)

## Environment

- Host: `Linux 7.0.11-1-cachyos x86_64`
- CPU: `11th Gen Intel(R) Core(TM) i7-11800H` (8C/16T)
- Python: `3.13.12`
- Rust: `1.92.0`
- uv: `0.11.19`
- Upstream PTT: `88429bb90acef55673f421f45038878809b1e577`
- Upstream RTN: `bdb9973109eb489be831af0c39bdf9c27e3378ed`
- This repo: `233f8d1a66e231791400da359960bcb5752fbf47`

## Python API: upstream vs this repo

Source data: [`python_vs_rust_2026-07-22.csv`](python_vs_rust_2026-07-22.csv). Each parser receives a 100-title untimed warm-up before measurement; both modes use the same Python executable.

| Parser | N | Upstream (items/s) | Rust port (items/s) | Speedup | Upstream p50 (ms) | Rust p50 (ms) | Upstream p95 (ms) | Rust p95 (ms) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `PTT.parse_title` | 1,000 | 1,788.9 | 14,024.8 | 7.84x | 0.565 | 0.071 | 0.592 | 0.082 |
| `RTN.parse` | 1,000 | 1,680.0 | 11,654.1 | 6.94x | 0.600 | 0.086 | 0.632 | 0.097 |
| `PTT.parse_title` | 10,000 | 1,791.9 | 13,852.4 | 7.73x | 0.560 | 0.071 | 0.607 | 0.085 |
| `RTN.parse` | 10,000 | 1,663.2 | 11,592.2 | 6.97x | 0.602 | 0.086 | 0.658 | 0.099 |
| `PTT.parse_title` | 30,000 | 1,825.2 | 14,327.4 | 7.85x | 0.550 | 0.070 | 0.588 | 0.080 |
| `RTN.parse` | 30,000 | 1,714.4 | 11,656.6 | 6.80x | 0.586 | 0.085 | 0.626 | 0.098 |

Geometric mean throughput speedup (all rows): **7.34x** (**7.81x** for PTT and **6.90x** for RTN).

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
