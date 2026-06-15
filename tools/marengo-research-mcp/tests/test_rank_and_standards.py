from marengo_research_mcp.humanoid.queries import expand_query
from marengo_research_mcp.humanoid.rank import dedupe_hits, rank_hits
from marengo_research_mcp.models import ResearchHit
from marengo_research_mcp.sources.standards import search_standards


def test_expand_query_includes_base():
    out = expand_query("humanoid locomotion")
    assert out[0] == "humanoid locomotion"
    assert len(out) >= 1


def test_dedupe_hits_by_url():
    hits = [
        ResearchHit(type="web", title="A", url="https://example.com/a"),
        ResearchHit(type="web", title="A dup", url="https://example.com/a/"),
    ]
    assert len(dedupe_hits(hits)) == 1


def test_rank_hits_prefers_title_match():
    hits = [
        ResearchHit(type="paper", title="Unrelated", url="https://a.com", source_name="arxiv"),
        ResearchHit(
            type="paper",
            title="Humanoid locomotion control",
            url="https://b.com",
            source_name="semantic_scholar",
        ),
    ]
    ranked = rank_hits(hits, "humanoid locomotion")
    assert "Humanoid" in ranked[0].title


def test_search_standards_returns_iso13482():
    hits = search_standards("personal care robot safety", limit=5)
    assert any("13482" in h.title for h in hits)
