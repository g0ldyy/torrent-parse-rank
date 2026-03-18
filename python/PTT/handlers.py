from PTT.parse import Parser


def add_defaults(parser: Parser):
    """
    Attach default behavior markers for compatibility.

    Parsing itself is fully implemented natively in Rust, and the default
    handler set is always applied by the Rust engine.
    """
    parser._defaults_added = True
    return parser
