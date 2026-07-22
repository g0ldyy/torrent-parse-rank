from pathlib import Path

import orjson
import PTT
import pytest
import regex
from PTT import adult, anime, cli, handlers, parse, transformers
from pydantic_core import PydanticSerializationError
from RTN import (
    RTN,
    BaseRankingModel,
    DefaultRanking,
    ParsedData,
    Resolution,
    SettingsModel,
    Torrent,
    check_fetch,
    check_fetch_and_rank,
    check_fetch_and_rank_many,
    check_pattern,
    episodes_from_season,
    get_lev_ratio,
    get_rank,
    get_resolution,
    normalize_title,
    sort_torrents,
    title_match,
)
from RTN import parse as rtn_parse
from RTN._native_bridge import data_to_json, rank_model_to_json, settings_to_json
from RTN.fetch import (
    adult_handler,
    check_exclude,
    check_required,
    fetch_audio,
    fetch_codec,
    fetch_hdr,
    fetch_other,
    fetch_quality,
    fetch_resolution,
    language_handler,
    populate_langs,
    trash_handler,
)
from RTN.models import LanguagesConfig
from RTN.patterns import _compile_patterns
from RTN.ranker import (
    calculate_audio_rank,
    calculate_channels_rank,
    calculate_codec_rank,
    calculate_extra_ranks,
    calculate_hdr_rank,
    calculate_preferred,
    calculate_preferred_langs,
    calculate_quality_rank,
)
from torrent_parse_rank_native import _native


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


def test_transformer_range_expansion_is_bounded():
    assert transformers.range_func("9999-10001") == [9999, 10000, 10001]
    assert transformers.range_func("1-10001") is None
    assert transformers.range_x_of_y_func("10000")[-1] == 10000
    assert transformers.range_x_of_y_func("10001") is None
    assert transformers.range_x_of_y_func("0") is None


def test_native_parser_rejects_oversized_of_range_without_allocating_it():
    parsed = parse.Parser().parse("Show [1000000000 of 1000000001]")
    assert parsed["episodes"] == []


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


def test_adult_keyword_loader_is_limited_to_owned_resources():
    for filename in ("../pyproject.toml", "/etc/passwd", "unknown.txt", None):
        with pytest.raises(ValueError, match="bundled PTT keyword"):
            adult.load_adult_keywords(filename)


def test_ptt_handler_configuration_uses_current_exact_schema():
    assert anime.anime_handler(parse.Parser()) is not None
    assert not hasattr(anime.anime_handler(parse.Parser()), "_anime_handler_added")
    with pytest.raises(TypeError, match="PTT Parser"):
        anime.anime_handler(object())

    assert parse.extend_options(None)["remove"] is False
    for options in ([], False, ""):
        with pytest.raises(TypeError, match="dictionary"):
            parse.extend_options(options)
    with pytest.raises(TypeError, match="booleans"):
        parse.extend_options({"remove": 1})
    with pytest.raises(ValueError, match="unknown"):
        parse.extend_options({"legacy": True})


def test_cli_sort_by_count_is_strict_and_deterministic(tmp_path: Path):
    source = tmp_path / "counts.txt"
    source.write_text("z,2\na,10\nb,2\n", encoding="utf-8")

    cli.sort_by_count(str(source))

    assert source.read_text(encoding="utf-8").splitlines() == ["a,10", "b,2", "z,2"]


def test_cli_sort_by_count_preserves_invalid_input(tmp_path: Path):
    source = tmp_path / "counts.txt"
    original = "valid,2\nmalformed\n"
    source.write_text(original, encoding="utf-8")

    with pytest.raises(ValueError, match="line 2"):
        cli.sort_by_count(str(source))

    assert source.read_text(encoding="utf-8") == original


def test_cli_combine_uses_only_regular_txt_sources(tmp_path: Path):
    (tmp_path / "b.txt").write_text("z\na\n", encoding="utf-8")
    (tmp_path / "a.txt").write_text("b\na\n", encoding="utf-8")
    (tmp_path / "combined-keywords-extra.txt").write_text("c\n", encoding="utf-8")
    (tmp_path / "ignored.txt").mkdir()

    cli.combine_keywords(str(tmp_path))

    assert (tmp_path / "combined-keywords.txt").read_text(encoding="utf-8").splitlines() == [
        "a",
        "b",
        "c",
        "z",
    ]


def test_cli_atomic_replacement_failure_preserves_source(tmp_path: Path, monkeypatch):
    source = tmp_path / "keywords.txt"
    original = "z\na\na\n"
    source.write_text(original, encoding="utf-8")

    def fail_replace(_source, _destination):
        raise OSError("replacement failed")

    monkeypatch.setattr(cli.os, "replace", fail_replace)
    with pytest.raises(OSError, match="replacement failed"):
        cli.dedupe_and_sort(str(source))

    assert source.read_text(encoding="utf-8") == original
    assert list(tmp_path.glob(".*.tmp")) == []


def test_cli_rejects_removed_anime_switch(monkeypatch):
    monkeypatch.setattr("sys.argv", ["ptt", "parse", "title", "--anime"])

    with pytest.raises(SystemExit) as error:
        cli.main()

    assert error.value.code == 2


def test_native_adult_keyword_detection():
    assert PTT.parse_title("Alexis Texas 2024 1080p WEB-DL")["adult"] is True
    assert "adult" not in PTT.parse_title("The.Matrix.1999.1080p.BluRay")


def test_native_numeric_boundaries_reject_booleans():
    with pytest.raises(TypeError, match="threshold.*bool"):
        _native.rtn_get_lev_ratio("title", "title", True)
    with pytest.raises(TypeError, match="threshold.*bool"):
        _native.rtn_title_match("title", "title", True)
    with pytest.raises(TypeError, match="season_num.*bool"):
        _native.rtn_episodes_from_season("Show.S01E01", True)


def test_native_json_boundaries_require_current_object_roots_and_titles():
    valid_data = '{"raw_title":"Movie.2026"}'
    invalid_calls = [
        lambda: _native.ptt_parse_title(""),
        lambda: _native.ptt_parse_many(["Movie.2026", ""]),
        lambda: _native.rtn_check_fetch("{}", "{}"),
        lambda: _native.rtn_check_fetch(valid_data, "[]"),
        lambda: _native.rtn_get_rank(valid_data, "{}", "[]"),
        lambda: _native.rtn_check_fetch_and_rank_many(["{}"], "{}", "{}"),
        lambda: _native.rtn_populate_langs("[]"),
    ]

    for call in invalid_calls:
        with pytest.raises(ValueError):
            call()

    for native_similarity in (_native.rtn_get_lev_ratio, _native.rtn_title_match):
        with pytest.raises(ValueError, match="Aliases must map"):
            native_similarity("Title", "Title", 0.85, '{"en":"Title"}')


def test_parsed_data_uses_one_strict_current_schema():
    parsed = rtn_parse("Movie.2026.3D.1080p.WEB-DL")

    assert parsed._3d is True
    assert parsed.model_dump(mode="json", by_alias=True)["_3d"] is True

    malformed = [
        {"raw_title": "Movie.2026", "title": "legacy intermediate"},
        {"raw_title": "Movie.2026", "trash": 1},
        {"raw_title": "Movie.2026", "seasons": [True]},
        {"raw_title": ""},
    ]
    for payload in malformed:
        with pytest.raises(ValueError):
            ParsedData.model_validate(payload)


def test_torrent_uses_one_strict_current_schema():
    parsed = ParsedData(raw_title="Movie.2026")
    valid = {
        "infohash": "a" * 40,
        "raw_title": "Movie.2026",
        "data": parsed,
    }

    malformed = [
        {**valid, "legacy": True},
        {**valid, "fetch": 1},
        {**valid, "seeders": True},
        {**valid, "leechers": -1},
        {**valid, "trackers": [""]},
        {**valid, "lev_ratio": float("nan")},
        {**valid, "lev_ratio": 1.1},
    ]
    for payload in malformed:
        with pytest.raises(ValueError):
            Torrent.model_validate(payload)


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


def test_settings_save_is_atomic_and_preserves_existing_file_on_failure(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
):
    path = tmp_path / "settings.json"
    path.write_text("original", encoding="utf-8")
    path.chmod(0o640)

    def fail_replace(source, destination):
        del source, destination
        raise OSError("replace failed")

    monkeypatch.setattr("RTN.models.os.replace", fail_replace)
    with pytest.raises(OSError, match="replace failed"):
        SettingsModel(name="updated").save(path)

    assert path.read_text(encoding="utf-8") == "original"
    assert path.stat().st_mode & 0o777 == 0o640
    assert list(tmp_path.glob(".settings.json.*.tmp")) == []


def test_add_defaults_rejects_non_parser_without_inert_marker():
    parser = parse.Parser()
    assert handlers.add_defaults(parser) is parser
    assert not hasattr(parser, "_defaults_added")

    with pytest.raises(TypeError, match="PTT Parser"):
        handlers.add_defaults(object())


def test_settings_reject_scalar_pattern_configuration():
    with pytest.raises(ValueError, match="list or tuple"):
        SettingsModel(require="silently-dropped-before")


def test_settings_and_ranking_reject_unknown_or_coerced_values():
    invalid_factories = [
        lambda: SettingsModel(legacy=True),
        lambda: SettingsModel(options={"legacy": True}),
        lambda: SettingsModel(options={"remove_ranks_under": True}),
        lambda: SettingsModel(options={"remove_all_trash": "false"}),
        lambda: SettingsModel(options={"title_similarity": "0.5"}),
        lambda: SettingsModel(options={"title_similarity": float("nan")}),
        lambda: SettingsModel(options={"title_similarity": 1.1}),
        lambda: SettingsModel(resolutions={"r1080p": 1}),
        lambda: SettingsModel(custom_ranks={"quality": {"remux": {"rank": True}}}),
        lambda: DefaultRanking(remux=True),
        lambda: DefaultRanking(legacy=1),
        lambda: BaseRankingModel(remux=True),
        lambda: BaseRankingModel(legacy=1),
    ]

    for factory in invalid_factories:
        with pytest.raises(ValueError):
            factory()


@pytest.mark.parametrize(
    "rank_function,needs_rank_model",
    [
        (get_rank, True),
        (calculate_preferred, False),
        (calculate_preferred_langs, False),
        (calculate_quality_rank, True),
        (calculate_codec_rank, True),
        (calculate_hdr_rank, True),
        (calculate_audio_rank, True),
        (calculate_channels_rank, True),
        (calculate_extra_ranks, True),
    ],
)
def test_rank_functions_enforce_current_model_boundaries(rank_function, needs_rank_model):
    data = ParsedData(raw_title="Movie.2026")
    settings = SettingsModel()
    rank_model = DefaultRanking()

    valid_args = (data, settings, rank_model) if needs_rank_model else (data, settings)
    assert type(rank_function(*valid_args)) is int

    invalid_calls = [
        (object(), settings, rank_model) if needs_rank_model else (object(), settings),
        (data, object(), rank_model) if needs_rank_model else (data, object()),
    ]
    if needs_rank_model:
        invalid_calls.append((data, settings, object()))

    for args in invalid_calls:
        with pytest.raises(TypeError):
            rank_function(*args)


def test_check_pattern_preserves_pattern_semantics_and_list_mutation():
    patterns = [None, regex.compile("HDR", regex.IGNORECASE)]
    assert check_pattern(patterns, "hdr")

    patterns[1] = regex.compile("DV")
    assert check_pattern(patterns, "DV")
    assert not check_pattern(patterns, "dv")
    assert check_pattern(["HDR"], "hdr")
    assert check_pattern(["HDR", "("], "HDR")

    with pytest.raises(ValueError):
        check_pattern(["("], "title")


def test_check_pattern_reuses_compiled_native_patterns():
    _compile_patterns.cache_clear()
    patterns = [regex.compile("WEB.?DL", regex.IGNORECASE), "REMUX"]

    assert check_pattern(patterns, "Movie.WEB-DL")
    assert check_pattern(patterns, "Movie.REMUX")

    cache = _compile_patterns.cache_info()
    assert cache.misses == 1
    assert cache.hits == 1


@pytest.mark.parametrize("patterns", [None, (), "HDR", {"HDR"}])
def test_check_pattern_rejects_non_list_roots(patterns):
    with pytest.raises(TypeError, match="Patterns must be a list"):
        check_pattern(patterns, "HDR")


@pytest.mark.parametrize("raw_title", [None, 1, False, ["HDR"]])
def test_check_pattern_rejects_non_string_titles(raw_title):
    with pytest.raises(TypeError, match="Raw title must be a string"):
        check_pattern(["HDR"], raw_title)


def test_normalize_title_preserves_trimmed_and_untrimmed_results():
    assert normalize_title("The.Matrix") == "the matrix"
    assert normalize_title("  Amélie & Friends  ") == "amelie and friends"


@pytest.mark.parametrize("raw_title", [None, 1, False, ["Title"]])
def test_normalize_title_rejects_non_string_titles(raw_title):
    with pytest.raises(TypeError, match="Raw title must be a string"):
        normalize_title(raw_title)


@pytest.mark.parametrize("lower", [None, 1, 0, "true"])
def test_normalize_title_rejects_non_boolean_lower(lower):
    with pytest.raises(TypeError, match="Lower must be a boolean"):
        normalize_title("Title", lower=lower)


@pytest.mark.parametrize("aliases", [[], "", False])
def test_title_similarity_rejects_non_mapping_aliases(aliases):
    with pytest.raises(TypeError, match="Aliases must be a dictionary or None"):
        get_lev_ratio("Title", "Title", aliases=aliases)


def test_missing_aliases_match_an_explicit_empty_mapping():
    assert get_lev_ratio("Title", "Title") == get_lev_ratio("Title", "Title", aliases={})


def test_title_similarity_rejects_boolean_threshold():
    with pytest.raises(ValueError, match="threshold"):
        title_match("Title", "Title", threshold=True)


@pytest.mark.parametrize(
    "aliases",
    [
        {"": ["Title"]},
        {"en": "Title"},
        {"en": [""]},
        {"en": [1]},
        {1: ["Title"]},
    ],
)
def test_title_similarity_rejects_non_current_alias_entries(aliases):
    with pytest.raises(TypeError, match="Aliases must map"):
        get_lev_ratio("Title", "Title", aliases=aliases)


def test_episode_extraction_rejects_boolean_season():
    with pytest.raises(TypeError, match="positive integer"):
        episodes_from_season("Show.S01E01", True)


@pytest.mark.parametrize("bucket_limit", [True, -1, 1.5, "2"])
def test_sort_torrents_rejects_invalid_bucket_limits(bucket_limit):
    with pytest.raises(TypeError, match="bucket limit"):
        sort_torrents(set(), bucket_limit=bucket_limit)


@pytest.mark.parametrize("resolutions", [False, (), [Resolution.FHD_1080P, "720p"]])
def test_sort_torrents_rejects_invalid_resolutions(resolutions):
    with pytest.raises(TypeError, match="Resolutions"):
        sort_torrents(set(), resolutions=resolutions)


def test_sort_torrents_keeps_zero_as_unbounded():
    assert sort_torrents(set(), bucket_limit=0) == {}


def test_sort_torrents_breaks_equal_rank_ties_deterministically():
    torrents = {
        Torrent(
            infohash=infohash,
            raw_title="Movie.1080p",
            data=ParsedData(raw_title="Movie.1080p", resolution="1080p"),
            rank=100,
        )
        for infohash in ("a" * 40, "c" * 40, "b" * 40)
    }

    assert list(sort_torrents(torrents)) == ["c" * 40, "b" * 40, "a" * 40]


def test_parser_entry_points_enforce_current_controls_and_models():
    settings = SettingsModel()
    ranking = DefaultRanking()
    rtn = RTN(settings, ranking)

    with pytest.raises(TypeError, match="Settings"):
        RTN(object())
    with pytest.raises(TypeError, match="Rank model"):
        RTN(settings, object())
    with pytest.raises(TypeError, match="Torrent"):
        get_resolution(object())

    for flag_name in ("translate_langs", "json"):
        with pytest.raises(TypeError):
            parse_kwargs = {flag_name: 1}
            rtn_parse("Movie.2026", **parse_kwargs)

    rank_args = ("Movie.2026.1080p", "a" * 40)
    for rank_kwargs in (
        {"correct_title": False},
        {"remove_trash": 1},
        {"speed_mode": 1},
        {"aliases": {"en": "Movie"}},
    ):
        with pytest.raises(TypeError):
            rtn.rank(*rank_args, **rank_kwargs)

    with pytest.raises(TypeError):
        rtn.rank(*rank_args, legacy=True)


def test_direct_model_json_matches_current_native_payloads():
    parsed = rtn_parse("Movie.2026.2160p.WEB-DL.DV.HDR10.HEVC.TrueHD.Atmos")
    ranking = DefaultRanking()

    assert orjson.loads(data_to_json(parsed)) == parsed.model_dump(mode="json", by_alias=True)
    assert orjson.loads(rank_model_to_json(ranking)) == ranking.model_dump(
        mode="json", by_alias=True
    )


def test_native_settings_payload_preserves_pattern_flags_without_changing_public_json():
    settings = SettingsModel(
        require=["WEB.?DL", "/CaseSensitive/"],
        exclude=["CAM"],
        preferred=["REMUX"],
    )

    native = orjson.loads(settings_to_json(settings))
    assert native["require"] == [
        {"pattern": "WEB.?DL", "ignore_case": True},
        {"pattern": "CaseSensitive", "ignore_case": False},
    ]
    assert orjson.loads(settings.model_dump_json())["require"] == [
        "WEB.?DL",
        "CaseSensitive",
    ]

    settings.require.append(None)
    assert orjson.loads(settings_to_json(settings))["require"][-1] is None
    settings.require.append(1)
    with pytest.raises(PydanticSerializationError, match="Unsupported pattern item type"):
        settings_to_json(settings)


def test_populated_language_groups_are_deterministically_ordered():
    settings = SettingsModel(
        languages=LanguagesConfig(
            exclude=["anime"],
            required=["common"],
            allowed=["fr", "anime"],
        )
    )

    populate_langs(settings)

    assert settings.languages.exclude == sorted(settings.languages.exclude)
    assert settings.languages.required == sorted(settings.languages.required)
    assert settings.languages.allowed == sorted(settings.languages.allowed)
    assert {"anime", "ja", "zh", "ko"} <= set(settings.languages.exclude)


@pytest.mark.parametrize(
    "fetch_function",
    [
        trash_handler,
        adult_handler,
        language_handler,
        check_exclude,
        fetch_resolution,
        fetch_audio,
        fetch_quality,
        fetch_codec,
        fetch_hdr,
        fetch_other,
    ],
)
def test_failed_key_fetch_helpers_enforce_current_boundaries(fetch_function):
    data = ParsedData(raw_title="Movie.2026")
    settings = SettingsModel()
    assert type(fetch_function(data, settings, set())) is bool

    for args in (
        (object(), settings, set()),
        (data, object(), set()),
        (data, settings, []),
        (data, settings, {1}),
    ):
        with pytest.raises(TypeError):
            fetch_function(*args)


def test_fetch_entry_points_reject_non_current_controls_and_roots():
    data = ParsedData(raw_title="Movie.2026")
    settings = SettingsModel()
    ranking = DefaultRanking()

    with pytest.raises(TypeError):
        check_required(object(), settings)
    with pytest.raises(TypeError):
        check_required(data, object())
    with pytest.raises(TypeError):
        populate_langs(object())

    for speed_mode in (1, 0, None, "true"):
        with pytest.raises(TypeError, match="Speed mode"):
            check_fetch(data, settings, speed_mode=speed_mode)
        with pytest.raises(TypeError, match="Speed mode"):
            check_fetch_and_rank(data, settings, ranking, speed_mode=speed_mode)
        with pytest.raises(TypeError, match="Speed mode"):
            check_fetch_and_rank_many([], settings, ranking, speed_mode=speed_mode)

    with pytest.raises(TypeError, match="iterable"):
        check_fetch_and_rank_many(object(), settings, ranking)
