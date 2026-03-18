#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PCRE2_SYS_STATIC="${PCRE2_SYS_STATIC:-1}"

"${ROOT_DIR}/scripts/fetch_upstream_tests.sh"

FIXTURE="${ROOT_DIR}/parity_tests/upstream/rtn/video/[Yameii] Mushoku Tensei - Jobless Reincarnation - S02E15 [English Dub] [CR WEB-DL 1080p] [6CD6B5CA].mkv"
if [[ ! -f "${FIXTURE}" ]]; then
  mkdir -p "$(dirname "${FIXTURE}")"
  if command -v ffmpeg >/dev/null 2>&1; then
    ffmpeg -y -f lavfi -i testsrc=size=128x72:rate=24 -f lavfi -i sine=frequency=1000:sample_rate=48000 -t 1 -c:v libx264 -pix_fmt yuv420p -c:a aac -b:a 96k "${FIXTURE}"
  else
    echo "ffmpeg not found and required fixture is missing: ${FIXTURE}" >&2
    exit 1
  fi
fi

export PYTHONPATH="${ROOT_DIR}/python"
pytest -q "${ROOT_DIR}/parity_tests/upstream/ptt" "${ROOT_DIR}/parity_tests/upstream/rtn" "${ROOT_DIR}/parity_tests/local" "$@"
