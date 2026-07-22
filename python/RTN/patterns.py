"""Additional parsing patterns and utilities for RTN."""

from functools import lru_cache
from typing import Any

from torrent_parse_rank_native._native import _RtnPatternSet, rtn_normalize_title

from ._native_bridge import pattern_key_to_json, pattern_list_key

translationTable: dict[str, Any] = {}


def normalize_title(raw_title: str, lower: bool = True) -> str:
    return rtn_normalize_title(raw_title, lower)


@lru_cache(maxsize=128)
def _compile_patterns(
    patterns: tuple[tuple[str, bool] | None, ...],
) -> _RtnPatternSet:
    return _RtnPatternSet(pattern_key_to_json(patterns))


def check_pattern(patterns: list, raw_title: str) -> bool:
    return _compile_patterns(pattern_list_key(patterns)).is_match(raw_title)
