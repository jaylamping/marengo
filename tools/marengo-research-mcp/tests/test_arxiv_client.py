"""Offline contract checks against the locked arxiv client API."""

from datetime import datetime, timezone
from importlib.metadata import version
from types import SimpleNamespace

import arxiv
import pytest

from marengo_research_mcp.sources import arxiv as source


def test_locked_api_shape():
    assert version("arxiv") == "4.0.0"
    assert not hasattr(arxiv.Search(), "results")
    assert callable(arxiv.Client().results)


@pytest.mark.parametrize("fail_after_first", [False, True])
def test_client_iteration_and_provider_failure(monkeypatch, fail_after_first):
    searches = []

    class Search:
        def __init__(self, **kwargs):
            searches.append(kwargs)

    class Client:
        def results(self, search):
            assert isinstance(search, Search)
            for index in range(2):
                if index and fail_after_first:
                    raise RuntimeError("next page unavailable")
                yield SimpleNamespace(
                    title=f" Robot\n{index} ", entry_id=f"https://arxiv.org/abs/{index}",
                    summary="x" * 600, published=datetime(2026, 9, 1, tzinfo=timezone.utc),
                    authors=[SimpleNamespace(name=str(i)) for i in range(7)],
                    pdf_url=f"https://arxiv.org/pdf/{index}",
                )

    monkeypatch.setattr(source, "arxiv", SimpleNamespace(
        Search=Search, Client=Client,
        SortCriterion=SimpleNamespace(SubmittedDate="submitted"),
    ))
    if fail_after_first:
        with pytest.raises(RuntimeError, match="next page unavailable"):
            source.search_arxiv("humanoid", 2)
    else:
        hits = source.search_arxiv("humanoid", 2)
        assert [hit.title for hit in hits] == ["Robot 0", "Robot 1"]
        assert [hit.url for hit in hits] == ["https://arxiv.org/abs/0", "https://arxiv.org/abs/1"]
        assert all(hit.type == "paper" and hit.source_name == "arxiv" for hit in hits)
        assert all(hit.year == 2026 and hit.authors == [str(i) for i in range(5)] for hit in hits)
        assert all(hit.snippet == "x" * 500 for hit in hits)
        assert [hit.pdf_url for hit in hits] == ["https://arxiv.org/pdf/0", "https://arxiv.org/pdf/1"]
    assert searches == [{"query": "cat:cs.RO AND (humanoid)", "max_results": 2, "sort_by": "submitted"}]
