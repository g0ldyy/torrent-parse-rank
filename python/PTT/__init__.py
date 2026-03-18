from . import handlers, parse, transformers
from .handlers import add_defaults
from .parse import Parser

_parser = Parser()
add_defaults(_parser)


def parse_title(raw_title: str, translate_languages: bool = False) -> dict:
    """
    Parse a torrent title with the default PTT handler set.

    :param raw_title: Raw torrent title.
    :param translate_languages: If true, return translated language names.
    :return: Parsed fields.
    """
    return _parser.parse(raw_title, translate_languages)


__all__ = ["Parser", "add_defaults", "parse", "parse_title", "handlers", "transformers"]
