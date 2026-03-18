#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FULL=0

if [[ "${1:-}" == "--full" ]]; then
  FULL=1
fi

cd "${ROOT_DIR}"
export UV_CACHE_DIR="${UV_CACHE_DIR:-${ROOT_DIR}/.uv-cache}"

if ! uv run pytest --version >/dev/null 2>&1; then
  echo "Missing dev dependency: pytest. Run: uv sync --group dev" >&2
  exit 2
fi
if ! uv run ruff --version >/dev/null 2>&1; then
  echo "Missing dev dependency: ruff. Run: uv sync --group dev" >&2
  exit 2
fi

echo "[1/6] Rust fmt check"
cargo fmt --all -- --check

echo "[2/6] Rust clippy"
cargo clippy --all-targets --all-features -- -D warnings

echo "[3/6] Rust check"
cargo check -q

echo "[4/6] Python format + lint"
uv run ruff format --check python scripts parity_tests/local
uv run ruff check python scripts parity_tests/local

echo "[5/6] Local API tests"
uv run pytest -q parity_tests/local/test_api_surface.py

if [[ "${FULL}" -eq 1 ]]; then
  echo "[6/6] Full parity tests (upstream fetched on main)"
  uv run ./scripts/run_parity_tests.sh
else
  echo "[6/6] Skipped upstream parity tests (pass --full to enable)"
fi
