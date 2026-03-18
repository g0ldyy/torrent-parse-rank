"""Parser module for parsing torrent titles and extracting metadata using RTN patterns."""

from typing import Any

from torrent_parse_rank_native._native import rtn_check_fetch_and_rank, rtn_parse

from ._native_bridge import data_settings_rank_to_json
from .exceptions import GarbageTorrent, SettingsDisabled
from .extras import get_lev_ratio
from .models import BaseRankingModel, DefaultRanking, ParsedData, SettingsModel, Torrent


class RTN:
    def __init__(self, settings: SettingsModel, ranking_model: BaseRankingModel | None = None):
        self.settings = settings
        self.ranking_model = ranking_model if ranking_model else DefaultRanking()
        self.lev_threshold = self.settings.options.get("title_similarity", 0.85)

    def rank(
        self,
        raw_title: str,
        infohash: str,
        correct_title: str = "",
        remove_trash: bool = False,
        speed_mode: bool = True,
        **kwargs,
    ) -> Torrent:
        if not self.settings.enabled:
            raise SettingsDisabled("Settings are disabled and cannot be used.")

        if not raw_title or not infohash:
            raise ValueError("Both the title and infohash must be provided.")

        if len(infohash) != 40:
            raise GarbageTorrent(
                "The infohash must be a valid SHA-1 hash and 40 characters in length."
            )

        parsed_data: ParsedData = parse(raw_title)

        lev_ratio = 0.0
        if correct_title:
            aliases = kwargs.get("aliases", {})
            lev_ratio = get_lev_ratio(
                correct_title, parsed_data.parsed_title, self.lev_threshold, aliases
            )
            if remove_trash and lev_ratio < self.lev_threshold:
                raise GarbageTorrent(
                    f"'{raw_title}' does not match the correct title. "
                    f"correct title: '{correct_title}', parsed title: '{parsed_data.parsed_title}'"
                )

        data_json, settings_json, rank_model_json = data_settings_rank_to_json(
            parsed_data, self.settings, self.ranking_model
        )
        is_fetchable, failed_keys, rank = rtn_check_fetch_and_rank(
            data_json, settings_json, rank_model_json, speed_mode
        )

        if remove_trash:
            if not is_fetchable:
                raise GarbageTorrent(
                    f"'{parsed_data.raw_title}' denied by: {', '.join(failed_keys)}"
                )

            if rank < self.settings.options["remove_ranks_under"]:
                raise GarbageTorrent(
                    f"'{raw_title}' does not meet the minimum rank requirement, got rank of {rank}"
                )

        return Torrent(
            infohash=infohash,
            raw_title=raw_title,
            data=parsed_data,
            fetch=is_fetchable,
            rank=rank,
            lev_ratio=lev_ratio,
        )


def parse(
    raw_title: str, translate_langs: bool = False, json: bool = False
) -> ParsedData | dict[str, Any]:
    if not raw_title or not isinstance(raw_title, str):
        raise TypeError("The input title must be a non-empty string.")

    parsed_data = ParsedData(**dict(rtn_parse(raw_title, translate_langs)))

    if json:
        return parsed_data.model_dump(mode="json")
    return parsed_data
