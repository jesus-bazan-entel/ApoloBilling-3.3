#!/usr/bin/env python3
"""
SIP Endpoint Connectivity Monitor

Probes SIP endpoints via sipsak OPTIONS and stores results in PostgreSQL.
Runs as a systemd service, probing all enabled trunks every interval.

Usage:
    python3 monitor.py [--interval 60] [--timeout 2] [--workers 8] [--retention-days 7] [--once]

Environment Variables:
    DATABASE_URL: PostgreSQL connection string (required)
        Format: postgresql://user:password@host:port/database

Dependencies:
    - sipsak (apt install sipsak)
    - psycopg2 (apt install python3-psycopg2)
    - iproute2 (for ip route get)
"""

import argparse
import logging
import os
import re
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass
from datetime import datetime
from typing import Dict, List, Optional, Tuple
from urllib.parse import urlparse

try:
    import psycopg2
    from psycopg2.extras import execute_values
except ImportError:
    print("ERROR: psycopg2 not installed. Run: apt install python3-psycopg2")
    sys.exit(1)

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
)
logger = logging.getLogger(__name__)


@dataclass
class Endpoint:
    """Represents a SIP endpoint to probe."""
    trunk_id: str
    name: str
    trunk_type: str
    ip: str
    port: int


@dataclass
class ProbeResult:
    """Result of a SIP OPTIONS probe."""
    endpoint: Endpoint
    ts: int
    status: str  # 'up' or 'down'
    resp_code: int
    rtt_ms: Optional[float]
    evidence: str


class EndpointMonitor:
    """Main monitor service class."""

    def __init__(
        self,
        database_url: str,
        interval: int = 60,
        timeout: int = 2,
        workers: int = 8,
        retention_days: int = 7,
    ):
        self.database_url = database_url
        self.interval = interval
        self.timeout = timeout
        self.workers = workers
        self.retention_days = retention_days
        self.cycles_for_purge = max(1, 3600 // interval)  # Purge every ~1 hour
        self.cycle_count = 0
        self._src_ip_cache: Dict[str, str] = {}

    def get_db_connection(self):
        """Create a new database connection."""
        parsed = urlparse(self.database_url)
        return psycopg2.connect(
            host=parsed.hostname,
            port=parsed.port or 5432,
            database=parsed.path.lstrip("/"),
            user=parsed.username,
            password=parsed.password,
        )

    def load_endpoints(self) -> List[Endpoint]:
        """Load enabled trunks from routing_trunks table."""
        endpoints = []
        conn = None
        try:
            conn = self.get_db_connection()
            with conn.cursor() as cur:
                cur.execute("""
                    SELECT id::text, name, trunk_type, host, port
                    FROM routing_trunks
                    WHERE enabled = true
                    ORDER BY name
                """)
                for row in cur.fetchall():
                    trunk_id, name, trunk_type, host, port = row
                    # Skip hostnames, we need IPs for probing
                    # If host is a hostname, resolve it
                    ip = self._resolve_host(host)
                    if ip:
                        endpoints.append(Endpoint(
                            trunk_id=trunk_id,
                            name=name,
                            trunk_type=trunk_type or "public",
                            ip=ip,
                            port=port or 5060,
                        ))
                    else:
                        logger.warning(f"Could not resolve host {host} for trunk {name}")
        except Exception as e:
            logger.error(f"Failed to load endpoints: {e}")
        finally:
            if conn:
                conn.close()
        return endpoints

    def _resolve_host(self, host: str) -> Optional[str]:
        """Resolve hostname to IP address. Returns host if already an IP."""
        # Check if already an IP
        ip_pattern = re.compile(r"^\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}$")
        if ip_pattern.match(host):
            return host

        # Try to resolve
        try:
            import socket
            return socket.gethostbyname(host)
        except Exception as e:
            logger.warning(f"Failed to resolve {host}: {e}")
            return None

    def get_source_ip(self, dest_ip: str) -> str:
        """Get the source IP that will be used to reach the destination."""
        if dest_ip in self._src_ip_cache:
            return self._src_ip_cache[dest_ip]

        try:
            result = subprocess.run(
                ["ip", "route", "get", dest_ip],
                capture_output=True,
                text=True,
                timeout=5,
            )
            if result.returncode == 0:
                match = re.search(r"\bsrc\s+([0-9.]+)", result.stdout)
                if match:
                    src_ip = match.group(1)
                    self._src_ip_cache[dest_ip] = src_ip
                    return src_ip
        except Exception as e:
            logger.warning(f"Failed to get source IP for {dest_ip}: {e}")

        # Fallback to a reasonable default
        return "0.0.0.0"

    def probe(self, endpoint: Endpoint, ts: int) -> ProbeResult:
        """
        Probe a single endpoint using sipsak.

        IMPORTANT: sipsak returns rc=1 even on success. We must parse the output
        to determine success, never rely on return code.
        """
        src_ip = self.get_source_ip(endpoint.ip)
        cmd = [
            "sipsak",
            "-vv",
            "-s", f"sip:{endpoint.ip}:{endpoint.port}",
            "-H", src_ip,
        ]

        evidence_lines = [
            f"=== Probe @ {datetime.utcfromtimestamp(ts).isoformat()}Z ===",
            f"Endpoint: {endpoint.name}",
            f"Target: {endpoint.ip}:{endpoint.port}",
            f"Source IP: {src_ip}",
            f"Command: {' '.join(cmd)}",
            "",
        ]

        status = "down"
        resp_code = 0
        rtt_ms = None

        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=self.timeout,
            )
            output = result.stdout + result.stderr
            evidence_lines.append("=== Output ===")
            evidence_lines.append(output if output.strip() else "(no output)")

            # Parse for SIP response code
            # Pattern: "SIP/2.0 200 OK" or similar
            code_match = re.search(r"SIP/2\.0\s+(\d{3})", output)
            if code_match:
                resp_code = int(code_match.group(1))
                status = "up"  # Any SIP response means connectivity

            # Parse for RTT
            # Pattern: "reply received after 12.345 ms"
            rtt_match = re.search(r"reply received after\s+([\d.]+)\s*ms", output, re.IGNORECASE)
            if rtt_match:
                rtt_ms = float(rtt_match.group(1))
                status = "up"

            # Also check for direct status indicators
            if "timeout" in output.lower() or "no response" in output.lower():
                status = "down"

        except subprocess.TimeoutExpired:
            evidence_lines.append("=== Timeout ===")
            evidence_lines.append(f"Probe timed out after {self.timeout} seconds")
            status = "down"
        except Exception as e:
            evidence_lines.append(f"=== Error ===")
            evidence_lines.append(str(e))
            status = "down"

        # Cap evidence at 8192 bytes
        evidence = "\n".join(evidence_lines)
        if len(evidence) > 8192:
            evidence = evidence[:8189] + "..."

        return ProbeResult(
            endpoint=endpoint,
            ts=ts,
            status=status,
            resp_code=resp_code,
            rtt_ms=rtt_ms,
            evidence=evidence,
        )

    def dispatcher_status(self) -> Dict[str, Tuple[bool, Optional[float]]]:
        """
        Query Kamailio dispatcher for cross-check.
        Returns dict: {ip: (is_active, estimated_ms)}

        This helps correct false-negatives from sipsak (which probes from
        ephemeral port, not :5060 like dispatcher does).
        """
        result = {}
        try:
            proc = subprocess.run(
                ["kamcmd", "dispatcher.list"],
                capture_output=True,
                text=True,
                timeout=10,
            )
            if proc.returncode != 0:
                return result

            # Parse dispatcher output
            # Looking for lines like:
            # URI: sip:10.35.13.100:5060
            # FLAGS: AP
            current_ip = None
            for line in proc.stdout.splitlines():
                line = line.strip()

                # Extract IP from URI
                uri_match = re.match(r"URI:\s*sip:([0-9.]+)", line)
                if uri_match:
                    current_ip = uri_match.group(1)
                    continue

                # Extract flags
                if current_ip and line.startswith("FLAGS:"):
                    flags = line.split(":", 1)[1].strip()
                    # 'A' = Active, 'I' = Inactive, 'T' = Testing, 'D' = Disabled
                    is_active = "A" in flags and "I" not in flags
                    result[current_ip] = (is_active, None)
                    current_ip = None

        except Exception as e:
            logger.debug(f"Dispatcher query failed (non-critical): {e}")

        return result

    def run_cycle(self):
        """Run a single probe cycle."""
        endpoints = self.load_endpoints()
        if not endpoints:
            logger.warning("No endpoints to probe")
            return

        ts = int(time.time())
        logger.info(f"Starting probe cycle with {len(endpoints)} endpoints")

        # Get dispatcher status for cross-check
        disp_status = self.dispatcher_status()

        # Probe all endpoints in parallel
        results: List[ProbeResult] = []
        with ThreadPoolExecutor(max_workers=self.workers) as executor:
            futures = {
                executor.submit(self.probe, ep, ts): ep
                for ep in endpoints
            }
            for future in as_completed(futures):
                try:
                    result = future.result()

                    # Cross-check with dispatcher
                    if result.status == "down":
                        disp_info = disp_status.get(result.endpoint.ip)
                        if disp_info and disp_info[0]:  # Dispatcher says active
                            # Correct false-negative
                            logger.debug(
                                f"Correcting false-down for {result.endpoint.name} "
                                "(dispatcher shows active)"
                            )
                            result.status = "up"
                            result.evidence = (
                                "[Corrected by dispatcher cross-check]\n"
                                + result.evidence
                            )

                    results.append(result)
                except Exception as e:
                    logger.error(f"Probe failed: {e}")

        # Insert results into database
        self.insert_results(results)

        # Count status
        up_count = sum(1 for r in results if r.status == "up")
        down_count = len(results) - up_count
        logger.info(f"Cycle complete: {up_count} up, {down_count} down")

        # Periodic purge
        self.cycle_count += 1
        if self.cycle_count >= self.cycles_for_purge:
            self.purge_old_records()
            self.cycle_count = 0

    def insert_results(self, results: List[ProbeResult]):
        """Bulk insert probe results into database."""
        if not results:
            return

        conn = None
        try:
            conn = self.get_db_connection()
            with conn.cursor() as cur:
                # Prepare values for bulk insert
                values = [
                    (
                        r.ts,
                        r.endpoint.trunk_id,
                        r.endpoint.name,
                        r.endpoint.trunk_type,
                        r.endpoint.ip,
                        r.endpoint.port,
                        r.status,
                        r.resp_code,
                        r.rtt_ms,
                        r.evidence,
                    )
                    for r in results
                ]

                execute_values(
                    cur,
                    """
                    INSERT INTO endpoint_monitor_probes
                    (ts, trunk_id, trunk_name, trunk_type, ip, port, status, resp_code, rtt_ms, evidence)
                    VALUES %s
                    """,
                    values,
                )
                conn.commit()
                logger.debug(f"Inserted {len(values)} probe records")
        except Exception as e:
            logger.error(f"Failed to insert results: {e}")
            if conn:
                conn.rollback()
        finally:
            if conn:
                conn.close()

    def purge_old_records(self):
        """Delete records older than retention period."""
        cutoff_ts = int(time.time()) - (self.retention_days * 86400)
        conn = None
        try:
            conn = self.get_db_connection()
            with conn.cursor() as cur:
                cur.execute(
                    "DELETE FROM endpoint_monitor_probes WHERE ts < %s",
                    (cutoff_ts,)
                )
                deleted = cur.rowcount
                conn.commit()
                if deleted > 0:
                    logger.info(f"Purged {deleted} records older than {self.retention_days} days")
        except Exception as e:
            logger.error(f"Failed to purge old records: {e}")
            if conn:
                conn.rollback()
        finally:
            if conn:
                conn.close()

    def run(self, once: bool = False):
        """Main run loop."""
        logger.info(f"Starting endpoint monitor (interval={self.interval}s, timeout={self.timeout}s)")

        while True:
            try:
                cycle_start = time.time()
                self.run_cycle()
                cycle_duration = time.time() - cycle_start

                if once:
                    logger.info("Single cycle complete (--once mode)")
                    break

                # Sleep for remaining interval time
                sleep_time = max(0, self.interval - cycle_duration)
                if sleep_time > 0:
                    time.sleep(sleep_time)
                elif cycle_duration > self.interval:
                    logger.warning(
                        f"Cycle took {cycle_duration:.1f}s, longer than interval "
                        f"({self.interval}s). Consider increasing --workers."
                    )

            except KeyboardInterrupt:
                logger.info("Shutting down...")
                break
            except Exception as e:
                logger.error(f"Cycle failed: {e}")
                if once:
                    break
                time.sleep(5)  # Brief pause before retry


def main():
    parser = argparse.ArgumentParser(
        description="SIP Endpoint Connectivity Monitor",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument(
        "--interval",
        type=int,
        default=60,
        help="Seconds between probe cycles (default: 60)",
    )
    parser.add_argument(
        "--timeout",
        type=int,
        default=2,
        help="Timeout per probe in seconds (default: 2)",
    )
    parser.add_argument(
        "--workers",
        type=int,
        default=8,
        help="Parallel probe workers (default: 8)",
    )
    parser.add_argument(
        "--retention-days",
        type=int,
        default=7,
        help="Days to retain probe history (default: 7)",
    )
    parser.add_argument(
        "--once",
        action="store_true",
        help="Run a single probe cycle and exit",
    )
    parser.add_argument(
        "--debug",
        action="store_true",
        help="Enable debug logging",
    )

    args = parser.parse_args()

    if args.debug:
        logging.getLogger().setLevel(logging.DEBUG)

    # Get database URL from environment
    database_url = os.environ.get("DATABASE_URL")
    if not database_url:
        logger.error("DATABASE_URL environment variable not set")
        logger.error("Format: postgresql://user:password@host:port/database")
        sys.exit(1)

    # Check for sipsak
    try:
        subprocess.run(["sipsak", "-V"], capture_output=True, timeout=5)
    except FileNotFoundError:
        logger.error("sipsak not found. Install with: apt install sipsak")
        sys.exit(1)

    monitor = EndpointMonitor(
        database_url=database_url,
        interval=args.interval,
        timeout=args.timeout,
        workers=args.workers,
        retention_days=args.retention_days,
    )

    monitor.run(once=args.once)


if __name__ == "__main__":
    main()
