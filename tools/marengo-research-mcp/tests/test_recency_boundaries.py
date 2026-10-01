"""Independent rolling-window expectations, with a fixed UTC clock."""
from datetime import datetime, timedelta, timezone

import pytest

from marengo_research_mcp.dates import provider_datetime
from marengo_research_mcp.models import ResearchHit
from marengo_research_mcp.tools.research import _filter_recency

NOW = datetime(2026, 1, 2, 12, tzinfo=timezone.utc)


def hit(stamp=None, **kwargs):
    return ResearchHit(type="paper", title="fixture", url="https://fixture.invalid", published_at=stamp, **kwargs)


@pytest.mark.parametrize("window,days", [("week", 7), ("month", 30), ("year", 365)])
def test_inclusive_rolling_window_crosses_year(window, days):
    cutoff = NOW - timedelta(days=days)
    values = [hit(cutoff), hit(NOW), hit(cutoff - timedelta(microseconds=1)),
              hit(NOW + timedelta(microseconds=1)), hit(year=2026)]
    assert _filter_recency(values, window, now=NOW) == values[:2]


def test_same_calendar_year_does_not_mean_last_week():
    now = datetime(2026, 9, 29, tzinfo=timezone.utc)
    assert _filter_recency([hit(NOW)], "week", now=now) == []


def test_timezone_preserved_through_wire_and_compared_as_utc():
    stamp = datetime.fromisoformat("2025-12-26T07:00:00-05:00")
    value = ResearchHit.model_validate_json(hit(stamp).model_dump_json())
    assert value.published_at.utcoffset() == timedelta(hours=-5)
    assert _filter_recency([value], "week", now=NOW) == [value]


def test_old_paper_update_does_not_make_a_new_publication():
    value = hit(NOW - timedelta(days=400), updated_at=NOW)
    assert _filter_recency([value], "week", now=NOW) == []
    value.type = "code"
    assert _filter_recency([value], "week", now=NOW) == [value]


def test_unrestricted_keeps_unknown_dates():
    values = [hit(), hit(NOW + timedelta(days=1))]
    assert _filter_recency(values, "any", now=NOW) == values


@pytest.mark.parametrize("value", [None, True, "invalid", "2026-01-02T12:00:00", float("inf"), 10**100])
def test_unknown_provider_dates(value):
    assert provider_datetime(value) is None


def test_provider_date_precision_and_epoch_units():
    assert provider_datetime("2026-01-02") == datetime(2026, 1, 2, tzinfo=timezone.utc)
    assert provider_datetime(NOW.timestamp()) == NOW
    assert provider_datetime(NOW.timestamp() * 1000, epoch_milliseconds=True) == NOW


def test_naive_clock_rejected():
    with pytest.raises(ValueError, match="timezone"):
        _filter_recency([], "week", now=datetime(2026, 1, 2))
