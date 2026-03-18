from pathlib import Path

import PTT
from PTT import adult, anime, cli, handlers, parse, transformers


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
