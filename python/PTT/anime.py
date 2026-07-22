from PTT.parse import Parser


def anime_handler(parser: Parser):
    """
    Validate a parser using the native anime detection pipeline.
    """
    if not isinstance(parser, Parser):
        raise TypeError("parser must be a PTT Parser instance")
    return parser
