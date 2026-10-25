"""
Per-minute rates of MariaDB's cumulative counters.

MariaDBMetrics stores Slow_queries and Innodb_deadlocks as MariaDB reports
them: running totals since the server started. Dashboards and health status
need what happened recently, so the rate is the difference against the
previous collector sample.
"""

from typing import Tuple

from sqlalchemy import desc
from sqlalchemy.orm import Session

from app.models.monitoring import MariaDBMetrics


def mariadb_rates(db: Session, latest: MariaDBMetrics) -> Tuple[int, int]:
    """(slow queries, deadlocks) per minute between `latest` and the sample
    before it; (0, 0) when there is no earlier sample"""
    prev = (
        db.query(MariaDBMetrics)
        .filter(MariaDBMetrics.timestamp < latest.timestamp)
        .order_by(desc(MariaDBMetrics.timestamp))
        .first()
    )
    if not prev:
        return 0, 0
    minutes = (latest.timestamp - prev.timestamp).total_seconds() / 60
    if minutes <= 0:
        return 0, 0

    def per_minute(cur, old) -> int:
        cur, old = cur or 0, old or 0
        # a counter lower than before means MariaDB restarted: all of `cur`
        # happened since then
        delta = cur - old if cur >= old else cur
        return round(delta / minutes)

    return (
        per_minute(latest.slow_queries, prev.slow_queries),
        per_minute(latest.innodb_deadlocks, prev.innodb_deadlocks),
    )
