"""Provider dates: explicit epoch units, aware timestamps, UTC date-only precision."""

from datetime import date, datetime, time, timezone
import math


def provider_datetime(value: object, *, epoch_milliseconds: bool = False) -> datetime | None:
    """Malformed and naive timestamps are unknown; dates mean midnight UTC."""
    try:
        if isinstance(value, bool):
            return None
        if isinstance(value, (int, float)):
            if not math.isfinite(value):
                return None
            return datetime.fromtimestamp(value / (1000 if epoch_milliseconds else 1), timezone.utc)
        if isinstance(value, datetime):
            parsed = value
        elif isinstance(value, str):
            if len(value) == 10:
                return datetime.combine(date.fromisoformat(value), time(), timezone.utc)
            parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
        else:
            return None
        return parsed if parsed.tzinfo is not None and parsed.utcoffset() is not None else None
    except (ValueError, OverflowError, OSError):
        return None
