<h1 align="center" id="title">torrent-parse-rank</h1>

<p align="center">
  High-performance torrent title parsing and ranking for Python, powered by Rust.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/python-3.10%2B-3776AB?style=flat-square&logo=python&logoColor=white" />
  <img src="https://img.shields.io/badge/rust-core-000000?style=flat-square&logo=rust&logoColor=white" />
  <img src="https://img.shields.io/badge/license-GPLv3-blue?style=flat-square" />
</p>

## Credits

- Upstream RTN reference: https://github.com/dreulavelle/rank-torrent-name
- Upstream PTT reference: https://github.com/dreulavelle/PTT

## What You Get

- Fast `PTT` parsing API for raw torrent names.
- `RTN` ranking/filtering API with configurable quality rules.
- Python-first usage (`from PTT import parse_title`, `from RTN import RTN`).
- Rust performance without changing your Python integration style.

## Install In Your Python Project

### Option 1: Local path

```bash
uv add --editable /absolute/path/to/torrent-parse-rank
```

### Option 2: Git dependency

```bash
uv add "torrent-parse-rank@git+https://github.com/g0ldyy/torrent-parse-rank"
```

Notes:

- Installing from source requires a Rust toolchain (`cargo`) available in `PATH`.
- The import names are `PTT` and `RTN`.

## Use It In 60 Seconds

```bash
uv init tpr-demo
cd tpr-demo
uv add --editable /absolute/path/to/torrent-parse-rank
uv run python - <<'PY'
from PTT import parse_title
from RTN import RTN
from RTN.models import DefaultRanking, SettingsModel

title = "The.Walking.Dead.S05E03.1080p.WEB-DL.DD5.1.H264-ASAP"
print(parse_title(title, False)["title"])

rtn = RTN(SettingsModel(), DefaultRanking())
item = rtn.rank(
    raw_title=title,
    infohash="c08a9ee8ce3a5c2c08865e2b05406273cabc97e7",
)
print({"fetch": item.fetch, "rank": item.rank, "parsed_title": item.data.parsed_title})
PY
```

## Quickstart

### 1) Parse a torrent title (PTT)

```python
from PTT import parse_title

data = parse_title("The.Simpsons.S01E01.1080p.BluRay.x265", False)
print(data["title"])      # The Simpsons
print(data["resolution"]) # 1080p
```

### 2) Parse + rank a candidate (RTN)

```python
from RTN import RTN
from RTN.models import DefaultRanking, SettingsModel

rtn = RTN(SettingsModel(), DefaultRanking())
item = rtn.rank(
    raw_title="The Walking Dead S05E03 720p x264-ASAP",
    infohash="c08a9ee8ce3a5c2c08865e2b05406273cabc97e7",
)

print(item.fetch)  # True/False after filters
print(item.rank)   # computed rank score
print(item.data.parsed_title)
```

### 3) Parse only (RTN)

```python
from RTN import parse

parsed = parse("Oppenheimer.2023.2160p.REMUX.DV.HDR10Plus.TrueHD.7.1.HEVC")
print(parsed.resolution)
print(parsed.codec)
```

## Development

```bash
cd torrent-parse-rank
uv sync --group dev
uv run maturin develop --release
```

## Quality / Tests

```bash
cd torrent-parse-rank
./scripts/quality_gate.sh
./scripts/quality_gate.sh --full
```

`--full` includes upstream parity tests.

## Benchmarks

```bash
cd torrent-parse-rank
uv run scripts/bench_compare.py
cargo bench -p ptt-core --bench ptt_bench
cargo bench -p rtn-core --bench rtn_bench
```

## Repository Layout

- `crates/ptt-core`: native parser core.
- `crates/rtn-core`: native ranking/fetch core.
- `python/torrent_parse_rank_native`: PyO3 extension module.
- `python/PTT`, `python/RTN`: Python compatibility/public APIs.
