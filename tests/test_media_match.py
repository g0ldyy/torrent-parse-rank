import csv
import gzip

from MediaMatch import MetadataIndex
from MediaMatch.api import match_title
from MediaMatch.importers import import_anime_mappings, import_imdb


def _tsv(path, headers, rows):
    with gzip.open(path, "wt", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=headers, delimiter="\t")
        writer.writeheader()
        writer.writerows(rows)


def test_imdb_import_and_v4_duplicate_year_ranking(tmp_path):
    basics = tmp_path / "basics.tsv.gz"
    akas = tmp_path / "akas.tsv.gz"
    eps = tmp_path / "episodes.tsv.gz"
    _tsv(basics, ["tconst", "titleType", "primaryTitle", "originalTitle", "isAdult", "startYear", "endYear", "runtimeMinutes", "genres"], [
        dict(tconst="tt0000001", titleType="movie", primaryTitle="Dune", originalTitle="Dune", isAdult="0", startYear="1984"),
        dict(tconst="tt0000002", titleType="movie", primaryTitle="Dune", originalTitle="Dune", isAdult="0", startYear="2021"),
        dict(tconst="tt0000003", titleType="tvSeries", primaryTitle="Dune", originalTitle="Dune", isAdult="0", startYear="2021"),
    ])
    _tsv(akas, ["titleId", "ordering", "title", "region", "language", "types", "attributes", "isOriginalTitle"], [
        dict(titleId="tt0000001", ordering="1", title="Dune", region="GB", language="en", types="original", attributes="", isOriginalTitle="1"),
        dict(titleId="tt0000002", ordering="1", title="Dune", region="US", language="en", types="", attributes="", isOriginalTitle="0"),
        dict(titleId="tt0000003", ordering="1", title="Dune", region="US", language="en", types="", attributes="", isOriginalTitle="0"),
    ])
    _tsv(eps, ["tconst", "parentTconst", "seasonNumber", "episodeNumber"], [
        dict(tconst="tt1000001", parentTconst="tt0000003", seasonNumber="3", episodeNumber="1")
    ])
    index = MetadataIndex(tmp_path / "index.sqlite")
    assert import_imdb(index, basics, akas, eps) > 0
    assert [m.imdb_id for m in index.search("Dune", 2021)] == ["tt0000002", "tt0000001"]
    assert index.search("Dune", series=True)[0].imdb_id == "tt0000003"
    assert index.search("Dune", series=True, required_max_season=5) == []


def test_anime_ids_and_api_use_existing_parser(tmp_path):
    file = tmp_path / "anime.tsv"
    file.write_text("imdb\tanidb\tmal\ttvdb\nanime\t123\t456\t789\n", encoding="utf-8")
    index = MetadataIndex(tmp_path / "index.sqlite")
    import_anime_mappings(index, file)
    assert index.anime_lookup("anidb", "123")["mal"] == "456"
    assert match_title(index, "Cowboy Bebop 1998") == []
