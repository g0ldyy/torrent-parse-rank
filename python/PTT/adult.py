from functools import cache
from pathlib import Path


@cache
def load_adult_keywords(filename: str = "combined-keywords.txt") -> set[str]:
    """Load adult keywords from bundled keyword files."""
    keywords_file = Path(__file__).parent / "keywords" / filename
    keywords = set()

    with open(keywords_file, encoding="utf-8") as f:
        for line in f:
            keyword = line.strip().lower()
            if keyword and not keyword.isspace():
                keywords.add(keyword)

    return keywords


def is_adult_content(context):
    """Mark `context['result']['adult']` when title contains adult keywords."""
    if "adult" in context["result"] and context["result"]["adult"]:
        return

    title_lower = context["title"].lower()
    if any(keyword in title_lower for keyword in load_adult_keywords()):
        context["result"]["adult"] = True
