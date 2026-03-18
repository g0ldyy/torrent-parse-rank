"""Extras module for additional RTN functionality."""

from enum import Enum

from torrent_parse_rank_native._native import (
    rtn_episodes_from_season,
    rtn_extract_episodes,
    rtn_extract_seasons,
    rtn_get_lev_ratio,
    rtn_title_match,
)

from ._native_bridge import aliases_to_json
from .models import Torrent


class Resolution(Enum):
    UHD_2160P = 9
    UHD_1440P = 7
    FHD_1080P = 6
    HD_720P = 5
    SD_576P = 4
    SD_480P = 3
    SD_360P = 2
    UNKNOWN = 1


RESOLUTION_MAP: dict[str, Resolution] = {
    "2160p": Resolution.UHD_2160P,
    "1440p": Resolution.UHD_1440P,
    "1080p": Resolution.FHD_1080P,
    "720p": Resolution.HD_720P,
    "576p": Resolution.SD_576P,
    "480p": Resolution.SD_480P,
    "360p": Resolution.SD_360P,
    "unknown": Resolution.UNKNOWN,
}


def _validate_raw_title(raw_title: str, exc_type: type[Exception] = TypeError) -> None:
    if not raw_title or not isinstance(raw_title, str):
        raise exc_type("The input title must be a non-empty string.")


def _validate_similarity_inputs(correct_title: str, parsed_title: str, threshold: float) -> None:
    if not (correct_title and parsed_title):
        raise ValueError("Both titles must be provided.")
    if not isinstance(threshold, (int, float)) or not 0 <= threshold <= 1:
        raise ValueError("The threshold must be a number between 0 and 1.")


def _validate_season_number(season_num: int) -> None:
    if not season_num:
        raise ValueError("The season number must be provided.")
    if not isinstance(season_num, int) or season_num <= 0:
        raise TypeError("The season number must be a positive integer.")


def get_resolution(torrent: Torrent) -> Resolution:
    return RESOLUTION_MAP.get(torrent.data.resolution.lower(), Resolution.UNKNOWN)


def title_match(
    correct_title: str,
    parsed_title: str,
    threshold: float = 0.85,
    aliases: dict | None = None,
) -> bool:
    _validate_similarity_inputs(correct_title, parsed_title, threshold)
    return rtn_title_match(correct_title, parsed_title, threshold, aliases_to_json(aliases or {}))


def get_lev_ratio(
    correct_title: str,
    parsed_title: str,
    threshold: float = 0.85,
    aliases: dict | None = None,
) -> float:
    _validate_similarity_inputs(correct_title, parsed_title, threshold)
    return rtn_get_lev_ratio(correct_title, parsed_title, threshold, aliases_to_json(aliases or {}))


def sort_torrents(
    torrents: set[Torrent],
    bucket_limit: int | None = None,
    resolutions: list[Resolution] | None = None,
) -> dict[str, Torrent]:
    resolutions = resolutions or []
    if not isinstance(torrents, set) or not all(isinstance(t, Torrent) for t in torrents):
        raise TypeError("The input must be a set of Torrent objects.")

    ranked: list[tuple[Resolution, Torrent]] = [
        (get_resolution(torrent), torrent) for torrent in torrents
    ]
    if resolutions:
        ranked = [item for item in ranked if item[0] in resolutions]

    ranked.sort(
        key=lambda item: (item[0].value, item[1].rank),
        reverse=True,
    )

    if bucket_limit and bucket_limit > 0:
        bucket_groups: dict[Resolution, list[Torrent]] = {}
        for resolution, torrent in ranked:
            bucket_groups.setdefault(resolution, []).append(torrent)

        result = {}
        for bucket_torrents in bucket_groups.values():
            for torrent in bucket_torrents[:bucket_limit]:
                result[torrent.infohash] = torrent
        return result

    return {torrent.infohash: torrent for _, torrent in ranked}


def extract_seasons(raw_title: str) -> list[int]:
    _validate_raw_title(raw_title)
    return [int(v) for v in rtn_extract_seasons(raw_title)]


def extract_episodes(raw_title: str) -> list[int]:
    _validate_raw_title(raw_title)
    return [int(v) for v in rtn_extract_episodes(raw_title)]


def episodes_from_season(raw_title: str, season_num: int) -> list[int]:
    _validate_season_number(season_num)
    _validate_raw_title(raw_title, ValueError)

    return [int(v) for v in rtn_episodes_from_season(raw_title, season_num)]
