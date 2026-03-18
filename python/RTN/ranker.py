"""Functions to rank parsed data based on user settings."""

from collections.abc import Callable

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

from ._native_bridge import data_settings_rank_to_json, data_settings_to_json
from .models import BaseRankingModel, ParsedData, SettingsModel


def _assert_parsed_data(data: ParsedData) -> None:
    if not isinstance(data, ParsedData):
        raise TypeError("Parsed data must be an instance of ParsedData.")
    if not data.raw_title:
        raise ValueError("Parsed data cannot be empty.")


def _call_rank_native(
    native_fn: Callable[..., int],
    data: ParsedData,
    settings: SettingsModel,
    rank_model: BaseRankingModel | None = None,
) -> int:
    if rank_model is None:
        data_json, settings_json = data_settings_to_json(data, settings)
        return int(native_fn(data_json, settings_json))
    data_json, settings_json, rank_model_json = data_settings_rank_to_json(
        data, settings, rank_model
    )
    return int(native_fn(data_json, settings_json, rank_model_json))


def _make_rank_component(
    name: str, native_fn: Callable[[str, str, str], int]
) -> Callable[[ParsedData, SettingsModel, BaseRankingModel], int]:
    def component(data: ParsedData, settings: SettingsModel, rank_model: BaseRankingModel) -> int:
        return _call_rank_native(native_fn, data, settings, rank_model)

    component.__name__ = name
    return component


def get_rank(data: ParsedData, settings: SettingsModel, rank_model: BaseRankingModel) -> int:
    _assert_parsed_data(data)
    return _call_rank_native(rtn_get_rank, data, settings, rank_model)


def calculate_preferred(data: ParsedData, settings: SettingsModel) -> int:
    return _call_rank_native(rtn_calculate_preferred, data, settings)


def calculate_preferred_langs(data: ParsedData, settings: SettingsModel) -> int:
    return _call_rank_native(rtn_calculate_preferred_langs, data, settings)


calculate_quality_rank = _make_rank_component("calculate_quality_rank", rtn_calculate_quality_rank)
calculate_codec_rank = _make_rank_component("calculate_codec_rank", rtn_calculate_codec_rank)
calculate_hdr_rank = _make_rank_component("calculate_hdr_rank", rtn_calculate_hdr_rank)
calculate_audio_rank = _make_rank_component("calculate_audio_rank", rtn_calculate_audio_rank)
calculate_channels_rank = _make_rank_component(
    "calculate_channels_rank", rtn_calculate_channels_rank
)
calculate_extra_ranks = _make_rank_component("calculate_extra_ranks", rtn_calculate_extra_ranks)
