import pytest
from PTT import parse_title, transformers
from RTN import ParsedData, SettingsModel, parse
from RTN.fetch import fetch_hdr
from RTN.models import BaseRankingModel
from RTN.ranker import calculate_hdr_rank


@pytest.mark.parametrize(
    ("raw", "title"),
    [
        ("The.Dual.2024.1080p.WEBRip.x264.mkv", "The Dual"),
        ("The.HDR.Experiment.2024.1080p.BluRay.x264.mkv", "The HDR Experiment"),
        ("The Net (1995)", "The Net"),
        ("The.Net.1995.1080p.BluRay.x264-GRP", "The Net"),
        ("The HDR Experiment", "The HDR Experiment"),
        ("1917", "1917"),
        ("2001", "2001"),
        ("1917.mkv", "1917"),
        ("2001.mp4", "2001"),
    ],
)
def test_ambiguous_title_words_are_not_metadata(raw, title):
    for data in (parse_title(raw), parse(raw, json=True)):
        assert data.get("title", data.get("parsed_title")) == title
        assert data.get("hdr", []) == []
        assert data.get("dubbed", False) is False
        assert data.get("site") is None
        assert data.get("episodes", []) == []


@pytest.mark.parametrize(
    ("notation", "episodes"),
    [
        ("S01E02E04", [2, 4]),
        ("S01E02E04E06", [2, 4, 6]),
        ("S01E02-E04", [2, 3, 4]),
        ("S01E02+E04", [2, 4]),
        ("S01E00E02", [0, 2]),
        ("1x02x04", [2, 4]),
    ],
)
def test_episode_lists_and_ranges_are_distinct(notation, episodes):
    raw = f"Show.{notation}.720p.HDTV.x264.mkv"
    assert parse_title(raw)["episodes"] == episodes
    assert parse(raw).episodes == episodes


@pytest.mark.parametrize(
    ("text", "expected"),
    [
        ("2e4", [2, 4]),
        ("2,4,6", [2, 4, 6]),
        ("2-4", [2, 3, 4]),
        ("2-E4E6", [2, 3, 4, 6]),
        ("1-10001", None),
        ("4-2", None),
        ("1 to 6", [1, 2, 3, 4, 5, 6]),
        ("1:3", [1, 2, 3]),
        ("1 and 3", [1, 3]),
        ("1 2 3", [1, 2, 3]),
        ("11 01", None),
    ],
)
def test_public_range_transform_preserves_explicit_lists(text, expected):
    assert transformers.range_func(text) == expected


def test_convert_is_adapted_to_the_strict_rtn_model():
    raw = "Better.Call.Saul.S03E04.CONVERT.720p.WEB.h264-TBS"
    assert parse_title(raw)["convert"] is True
    parsed = parse(raw)
    assert parsed.converted is True
    assert parsed.parsed_title == "Better Call Saul"
    assert "convert" not in parse(raw, json=True)


@pytest.mark.parametrize(
    ("tags", "hdr"),
    [
        ("HDR", ["HDR"]),
        ("HDR10", ["HDR10"]),
        ("HLG", ["HLG"]),
        ("DV.HDR10", ["DV", "HDR10"]),
        ("HDR10Plus", ["HDR10+"]),
    ],
)
def test_hdr_formats_are_preserved(tags, hdr):
    raw = f"Aurora.2024.2160p.BluRay.x265.{tags}.mkv"
    assert parse_title(raw)["hdr"] == hdr
    assert parse(raw).hdr == hdr


def test_hdr_detail_uses_existing_hdr_policy_without_double_counting():
    data = ParsedData(raw_title="Aurora", hdr=["HDR", "HDR10", "HLG"])
    settings = SettingsModel()
    ranking = BaseRankingModel(hdr=123)
    assert calculate_hdr_rank(data, settings, ranking) == 123
    settings.custom_ranks.hdr.hdr.fetch = False
    assert fetch_hdr(data, settings, set()) is True


@pytest.mark.parametrize(
    "raw",
    [
        "Aurora.2024.DUAL.1080p.WEB-DL.x264.mkv",
        "[Group] Aurora.2024.1080p.WEB-DL.x264.[Dual Audio].mkv",
    ],
)
def test_real_dual_audio_tags_are_still_recognized(raw):
    assert parse_title(raw)["dubbed"] is True


def test_explicit_site_prefix_is_still_removed():
    parsed = parse_title("www.example.com - Aurora.2024.1080p.WEB-DL.x264.mkv")
    assert parsed["title"] == "Aurora"
    assert parsed["site"] == "www.example.com"


@pytest.mark.parametrize("raw", ["Rutracker.org", "BEST-TORRENTS.COM", "Ex-torrenty.org"])
def test_standalone_domain_is_still_a_site(raw):
    parsed = parse_title(raw)
    assert parsed["title"] == ""
    assert "site" in parsed


def test_zero_padded_absolute_episode_is_still_recognized():
    assert parse_title("0001.mkv")["episodes"] == [1]


def test_mixed_script_word_is_not_silently_shortened():
    raw = "АBullet.Train.2022.2160p.WEB-DL.x265-GRP"
    assert parse_title(raw)["title"] == "АBullet Train"


@pytest.mark.parametrize(
    ("raw", "title", "seasons", "episodes"),
    [
        ("NCIS Season 11 01.mp4", "NCIS", [11], [1]),
        ("Chernobyl E02 1 23 45.mp4", "Chernobyl", [], [2]),
        (
            "Watch Gary And His Demons Episode 10 - 0.00.07-0.11.02.mp4",
            "Watch Gary And His Demons",
            [],
            [10],
        ),
        (
            "Dragon Ball Z Movie - 09 - Bojack Unbound - 1080p BluRay x264 DTS 5.1 -DDR",
            "Dragon Ball Z Movie - 09 - Bojack Unbound",
            [],
            [],
        ),
    ],
)
def test_unmarked_numbers_do_not_override_explicit_structure(raw, title, seasons, episodes):
    data = parse_title(raw)
    assert data["title"] == title
    assert data["seasons"] == seasons
    assert data["episodes"] == episodes
