#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CACHE_DIR="${UPSTREAM_TEST_CACHE_DIR:-${ROOT_DIR}/.upstream-tests-cache}"
OUT_DIR="${UPSTREAM_TEST_OUT_DIR:-${ROOT_DIR}/parity_tests/upstream}"

PTT_UPSTREAM_URL="${PTT_UPSTREAM_URL:-https://github.com/dreulavelle/PTT.git}"
RTN_UPSTREAM_URL="${RTN_UPSTREAM_URL:-https://github.com/dreulavelle/rank-torrent-name.git}"
PTT_UPSTREAM_BRANCH="${PTT_UPSTREAM_BRANCH:-main}"
RTN_UPSTREAM_BRANCH="${RTN_UPSTREAM_BRANCH:-main}"

fetch_repo() {
  local name="$1"
  local url="$2"
  local branch="$3"
  local repo_dir="${CACHE_DIR}/${name}"

  if [[ -d "${repo_dir}/.git" ]]; then
    git -C "${repo_dir}" fetch --depth 1 origin "${branch}"
    git -C "${repo_dir}" checkout -B "${branch}" "origin/${branch}"
  else
    git clone --depth 1 --branch "${branch}" "${url}" "${repo_dir}"
  fi
}

mkdir -p "${CACHE_DIR}" "${OUT_DIR}/ptt" "${OUT_DIR}/rtn"

fetch_repo "PTT" "${PTT_UPSTREAM_URL}" "${PTT_UPSTREAM_BRANCH}"
fetch_repo "RTN" "${RTN_UPSTREAM_URL}" "${RTN_UPSTREAM_BRANCH}"

rsync -a --delete --exclude '__pycache__' "${CACHE_DIR}/PTT/tests/" "${OUT_DIR}/ptt/"
rsync -a --delete --exclude '__pycache__' "${CACHE_DIR}/RTN/tests/" "${OUT_DIR}/rtn/"

find "${OUT_DIR}" -type f \( -name '*.pyc' -o -name '*.pyo' \) -delete

echo "Fetched upstream tests to: ${OUT_DIR}"
