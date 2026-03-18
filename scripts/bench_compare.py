#!/usr/bin/env python3
import json
import os
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


@dataclass
class BenchResult:
    mode: str
    parser: str
    n: int
    total_s: float
    throughput: float
    p50_ms: float
    p95_ms: float


ROOT = Path(__file__).resolve().parents[1]
PORT_ROOT = ROOT.parent
UPSTREAM_PTT = PORT_ROOT / "PTT"
UPSTREAM_RTN = PORT_ROOT / "rank-torrent-name"
RUST_PY = ROOT / "python"
UPSTREAM_DEPS = [
    "arrow>=1.3.0,<2",
    "pydantic>=2",
    "pymediainfo>=7",
    "orjson>=3",
    "levenshtein>=0.27",
    "parsett>=1.8.2",
]


def _bench_code() -> str:
    return r"""
import json
import random
import statistics
import time

from PTT import parse_title
from RTN import parse as rtn_parse


def make_titles(n: int):
    base = [
        "The.Walking.Dead.S05E03.1080p.WEB-DL.DD5.1.H264-ASAP",
        "Oppenheimer.2023.2160p.REMUX.DV.HDR10Plus.TrueHD.7.1.HEVC",
        "Game.of.Thrones.S01E01.720p.HDTV.x264",
        "The.Simpsons.S01E01E02.1080p.BluRay.x265.10bit.AAC.5.1",
        "House.MD.All.Seasons.1-8.720p.Ultra-Compressed",
    ]
    random.seed(1337)
    out = []
    for i in range(n):
        t = random.choice(base)
        out.append(f"{t}.{i:05d}")
    return out


def run_bench(name, n, fn, titles):
    lat = []
    start = time.perf_counter()
    for t in titles:
        t0 = time.perf_counter()
        fn(t)
        lat.append((time.perf_counter() - t0) * 1000)
    total = time.perf_counter() - start
    return {
        "parser": name,
        "n": n,
        "total_s": total,
        "throughput": n / total if total > 0 else 0.0,
        "p50_ms": statistics.median(lat),
        "p95_ms": statistics.quantiles(lat, n=20)[18] if len(lat) >= 20 else max(lat),
    }


n = int(__import__('os').environ['BENCH_N'])
titles = make_titles(n)
out = [
    run_bench("PTT.parse_title", n, lambda t: parse_title(t, False), titles),
    run_bench("RTN.parse", n, rtn_parse, titles),
]
print(json.dumps(out))
"""


def run_mode(mode: str, n: int) -> list[BenchResult]:
    env = os.environ.copy()
    if mode == "rust":
        env["PYTHONPATH"] = str(RUST_PY)
        command = [sys.executable, "-c", _bench_code()]
    elif mode == "upstream":
        env["PYTHONPATH"] = os.pathsep.join([str(UPSTREAM_PTT), str(UPSTREAM_RTN)])
        command = ["uv", "run"]
        for dep in UPSTREAM_DEPS:
            command.extend(["--with", dep])
        command.extend(["python", "-c", _bench_code()])
    else:
        raise ValueError(mode)
    env["BENCH_N"] = str(n)

    try:
        proc = subprocess.run(
            command,
            cwd="/tmp",
            env=env,
            capture_output=True,
            text=True,
            check=True,
        )
    except subprocess.CalledProcessError as exc:
        stdout = (exc.stdout or "").strip()
        stderr = (exc.stderr or "").strip()
        details = stderr or stdout or str(exc)

        missing = re.search(r"No module named ['\"]([^'\"]+)['\"]", details)
        if mode == "upstream" and missing:
            pkg = missing.group(1)
            raise RuntimeError(
                "Upstream benchmark dependencies failed to resolve "
                f"(missing module '{pkg}').\n\n"
                f"Original error:\n{details}"
            ) from exc

        raise RuntimeError(f"Benchmark subprocess failed in mode='{mode}'.\n{details}") from exc
    rows = json.loads(proc.stdout)
    return [
        BenchResult(
            mode=mode,
            parser=row["parser"],
            n=row["n"],
            total_s=row["total_s"],
            throughput=row["throughput"],
            p50_ms=row["p50_ms"],
            p95_ms=row["p95_ms"],
        )
        for row in rows
    ]


def main() -> None:
    try:
        sizes = [1000, 10000, 30000]
        all_rows: list[BenchResult] = []
        for n in sizes:
            all_rows.extend(run_mode("upstream", n))
            all_rows.extend(run_mode("rust", n))

        print("mode,parser,n,total_s,throughput,p50_ms,p95_ms")
        for r in all_rows:
            print(
                f"{r.mode},{r.parser},{r.n},{r.total_s:.6f},{r.throughput:.1f},{r.p50_ms:.3f},{r.p95_ms:.3f}"
            )
    except RuntimeError as exc:
        print(exc, file=sys.stderr)
        raise SystemExit(2) from None


if __name__ == "__main__":
    main()
