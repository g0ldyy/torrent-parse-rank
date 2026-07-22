from PTT.parse import Parser


def add_defaults(parser: Parser):
    """Validate and return a parser using the native default handler set."""
    if not isinstance(parser, Parser):
        raise TypeError("parser must be a PTT Parser instance")
    return parser
