from pathlib import Path

import PTT
import pytest
import regex
from PTT import adult, anime, cli, handlers, parse, transformers
from RTN import (
    DefaultRanking,
    SettingsModel,
    check_fetch,
    check_fetch_and_rank,
    check_fetch_and_rank_many,
    get_rank,
)
from RTN import parse as rtn_parse


def test_api_modules_and_symbols_present():
    assert hasattr(PTT, "parse_title")
    assert hasattr(adult, "load_adult_keywords")
    assert hasattr(adult, "is_adult_content")
    assert hasattr(anime, "anime_handler")
    assert hasattr(cli, "main")
    assert hasattr(cli, "combine_keywords")
    assert hasattr(cli, "sort_by_count")
    assert hasattr(cli, "dedupe_and_sort")
    assert hasattr(handlers, "add_defaults")


def test_parse_module_symbols_present():
    for name in [
        "NON_ENGLISH_CHARS",
        "CURLY_BRACKETS",
        "SQUARE_BRACKETS",
        "PARENTHESES",
        "BRACKETS",
        "RUSSIAN_CAST_REGEX",
        "ALT_TITLES_REGEX",
        "NOT_ONLY_NON_ENGLISH_REGEX",
        "NOT_ALLOWED_SYMBOLS_AT_START_AND_END",
        "REMAINING_NOT_ALLOWED_SYMBOLS_AT_START_AND_END",
        "REDUNDANT_SYMBOLS_AT_END",
        "EMPTY_BRACKETS_REGEX",
        "PARANTHESES_WITHOUT_CONTENT",
        "MOVIE_REGEX",
        "STAR_REGEX_1",
        "STAR_REGEX_2",
        "MP3_REGEX",
        "SPACING_REGEX",
        "SPECIAL_CHAR_SPACING",
        "SUB_PATTERN",
        "BEFORE_TITLE_MATCH_REGEX",
        "DEBUG_HANDLER",
        "LANGUAGES_TRANSLATION_TABLE",
        "extend_options",
        "create_handler_from_regexp",
        "clean_title",
        "translate_langs",
        "Parser",
    ]:
        assert hasattr(parse, name), f"missing parse symbol: {name}"


def test_public_ptt_regex_constants_keep_upstream_semantics():
    assert parse.RUSSIAN_CAST_REGEX.search("Title (Русский)")
    assert not parse.RUSSIAN_CAST_REGEX.search("plain title")
    assert parse.NOT_ONLY_NON_ENGLISH_REGEX.search("English Русский текст")
    assert not parse.NOT_ONLY_NON_ENGLISH_REGEX.search("plain title")


def test_transformers_symbols_present():
    for name in [
        "none",
        "value",
        "integer",
        "first_integer",
        "boolean",
        "lowercase",
        "uppercase",
        "convert_months",
        "date",
        "range_func",
        "range_x_of_y_func",
        "year_range",
        "array",
        "uniq_concat",
        "transform_resolution",
    ]:
        assert hasattr(transformers, name), f"missing transformer symbol: {name}"


def test_parser_class_methods_present_and_working():
    parser = parse.Parser()
    handlers.add_defaults(parser)
    parser.add_handler("dummy", lambda context: None)
    out = parser.parse("The.Matrix.1999.1080p.BluRay.x264")
    assert out["title"] == "The Matrix"


def test_adult_keyword_loading_and_helpers(tmp_path: Path):
    keywords = adult.load_adult_keywords()
    assert isinstance(keywords, set)
    assert len(keywords) > 0

    context = {"title": "some normal title", "result": {}}
    adult.is_adult_content(context)
    assert "adult" not in context["result"]

    source = tmp_path / "source.txt"
    source.write_text("z\na\na\n", encoding="utf-8")
    cli.dedupe_and_sort(str(source))
    assert source.read_text(encoding="utf-8").splitlines() == ["a", "z"]


def test_native_adult_keyword_detection():
    assert PTT.parse_title("Alexis Texas 2024 1080p WEB-DL")["adult"] is True
    assert "adult" not in PTT.parse_title("The.Matrix.1999.1080p.BluRay")


def test_combined_fetch_and_rank_matches_individual_calls():
    data = rtn_parse("Oppenheimer.2023.2160p.REMUX.DV.HDR10Plus.TrueHD.7.1.HEVC")
    settings = SettingsModel()
    ranking = DefaultRanking()

    fetchable, failed_keys, rank = check_fetch_and_rank(data, settings, ranking)

    assert (fetchable, failed_keys) == check_fetch(data, settings)
    assert rank == get_rank(data, settings, ranking)


def test_batched_fetch_and_rank_matches_single_calls():
    data_items = [
        rtn_parse("The.Matrix.1999.1080p.BluRay.x264.DTS"),
        rtn_parse("Some.Movie.2020.CAM.XVID.MP3"),
        rtn_parse("Show.S02E03.2160p.WEB-DL.DV.HDR.HEVC"),
    ]
    settings = SettingsModel()
    ranking = DefaultRanking()

    actual = check_fetch_and_rank_many(data_items, settings, ranking)
    expected = [check_fetch_and_rank(data, settings, ranking) for data in data_items]

    assert actual == expected
    assert check_fetch_and_rank_many([], settings, ranking) == []


def test_batched_fetch_and_rank_matches_single_calls_with_patterns():
    data_items = [
        rtn_parse("The.Matrix.1999.1080p.BluRay.x264.DTS"),
        rtn_parse("Some.Movie.2020.MULTI.FRENCH.CAM.XVID.MP3"),
        rtn_parse("Show.S02E03.JAPANESE.2160p.WEB-DL.DV.HDR.HEVC"),
    ]
    settings = SettingsModel(
        require=[r"(?:matrix|movie|show)"],
        exclude=[r"(?:sample|password)$"],
        preferred=[r"(?:web.?dl|hevc|x265)"],
        languages={
            "allowed": ["fr"],
            "exclude": ["anime", "non_anime"],
            "preferred": ["fr", "ja"],
        },
    )
    ranking = DefaultRanking()

    actual = check_fetch_and_rank_many(data_items, settings, ranking, speed_mode=False)
    expected = [
        check_fetch_and_rank(data, settings, ranking, speed_mode=False) for data in data_items
    ]

    assert actual == expected


def test_settings_pattern_round_trip_preserves_case_sensitivity(tmp_path: Path):
    path = tmp_path / "settings.json"
    settings = SettingsModel(require=["/CaseSensitive/", "insensitive"])

    settings.save(path)
    loaded = SettingsModel.load(path)

    sensitive, insensitive = loaded.require
    assert not sensitive.flags & regex.IGNORECASE
    assert sensitive.search("CaseSensitive")
    assert not sensitive.search("casesensitive")
    assert insensitive.flags & regex.IGNORECASE
    assert insensitive.search("INSENSITIVE")


def test_settings_reject_scalar_pattern_configuration():
    with pytest.raises(ValueError, match="list or tuple"):
        SettingsModel(require="silently-dropped-before")
