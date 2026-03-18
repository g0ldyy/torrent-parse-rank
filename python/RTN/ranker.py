"""Functions to rank parsed data based on user settings."""

from torrent_parse_rank_native._native import (
    rtn_calculate_audio_rank,
    rtn_calculate_channels_rank,
    rtn_calculate_codec_rank,
    rtn_calculate_extra_ranks,
    rtn_calculate_hdr_rank,
    rtn_calculate_preferred,
    rtn_calculate_preferred_langs,
    rtn_calculate_quality_rank,
    rtn_get_rank,
)

from ._native_bridge import data_to_json, rank_model_to_json, settings_to_json
from .models import BaseRankingModel, ParsedData, SettingsModel


def get_rank(data: ParsedData, settings: SettingsModel, rank_model: BaseRankingModel) -> int:
    if not isinstance(data, ParsedData):
        raise TypeError("Parsed data must be an instance of ParsedData.")
    if not data.raw_title:
        raise ValueError("Parsed data cannot be empty.")
    return int(
        rtn_get_rank(
            data_to_json(data),
            settings_to_json(settings),
            rank_model_to_json(rank_model),
        )
    )


def calculate_preferred(data: ParsedData, settings: SettingsModel) -> int:
    return int(rtn_calculate_preferred(data_to_json(data), settings_to_json(settings)))


def calculate_preferred_langs(data: ParsedData, settings: SettingsModel) -> int:
    return int(rtn_calculate_preferred_langs(data_to_json(data), settings_to_json(settings)))


def calculate_quality_rank(
    data: ParsedData, settings: SettingsModel, rank_model: BaseRankingModel
) -> int:
    return int(
        rtn_calculate_quality_rank(
            data_to_json(data),
            settings_to_json(settings),
            rank_model_to_json(rank_model),
        )
    )


def calculate_codec_rank(
    data: ParsedData, settings: SettingsModel, rank_model: BaseRankingModel
) -> int:
    return int(
        rtn_calculate_codec_rank(
            data_to_json(data),
            settings_to_json(settings),
            rank_model_to_json(rank_model),
        )
    )


def calculate_hdr_rank(
    data: ParsedData, settings: SettingsModel, rank_model: BaseRankingModel
) -> int:
    return int(
        rtn_calculate_hdr_rank(
            data_to_json(data),
            settings_to_json(settings),
            rank_model_to_json(rank_model),
        )
    )


def calculate_audio_rank(
    data: ParsedData, settings: SettingsModel, rank_model: BaseRankingModel
) -> int:
    return int(
        rtn_calculate_audio_rank(
            data_to_json(data),
            settings_to_json(settings),
            rank_model_to_json(rank_model),
        )
    )


def calculate_channels_rank(
    data: ParsedData, settings: SettingsModel, rank_model: BaseRankingModel
) -> int:
    return int(
        rtn_calculate_channels_rank(
            data_to_json(data),
            settings_to_json(settings),
            rank_model_to_json(rank_model),
        )
    )


def calculate_extra_ranks(
    data: ParsedData, settings: SettingsModel, rank_model: BaseRankingModel
) -> int:
    return int(
        rtn_calculate_extra_ranks(
            data_to_json(data),
            settings_to_json(settings),
            rank_model_to_json(rank_model),
        )
    )
