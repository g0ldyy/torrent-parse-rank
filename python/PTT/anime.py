from PTT.parse import Parser


def anime_handler(parser: Parser):
    """
    Compatibility hook for anime handler registration.

    Rust core already includes the complete anime detection pipeline.
    """
    parser._anime_handler_added = True
    return parser
