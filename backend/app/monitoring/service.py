#!/usr/bin/env python3
"""
Updated Monitoring Service - Working collectors for actual SBC components
"""
import asyncio
import json
import os
import shlex
import subprocess
import time
import socket
import re
import uuid
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timedelta
from typing import Dict, Any, Optional, List
from dataclasses import dataclass

import psutil
import mysql.connector
from mysql.connector import Error as MySQLError

from app.db.session import SessionLocal
from sqlalchemy import func as sa_func, text as sa_text
from app.models.monitoring import (
    KamailioMetrics, RtpengineMetrics, MariaDBMetrics,
    SystemMetrics, MonitoringAlert, MonitoringConfig,
    MonitoringKpiHourly, MonitoringKpiDaily,
)

# Carrier-class history tiers (Vodafone/AT&T style):
#   raw 1-min  -> retention_days from monitoring_config (default 30d)
#   hourly     -> 93 days  (quarter)
#   daily      -> 400 days (13+ months)
# raw_data JSONB (heavy) is stripped after 48h; numeric columns are kept.
HOURLY_RETENTION_DAYS = 93
DAILY_RETENTION_DAYS = 400

# Trunk health probe (KamailioCollector._probe_trunks)
KAMAILIO_CFG = "/etc/kamailio/kamailio.cfg"
ONE_SIDED_MIN_AGE = 90          # s; younger sessions may still be ringing
TRUNK_PROBE_TIMEOUT = 4          # seconds per SIP OPTIONS
TRUNK_LOG_RETENTION_DAYS = 7     # routing_sip_status_log: 1 row/trunk/minute
RAW_JSON_STRIP_HOURS = 48

# Curated KPI lists per component for rollups (must match existing columns).
ROLLUP_METRICS = {
    "kamailio": [
        "active_dialogs", "active_calls", "early_dialogs", "confirmed_dialogs",
        "invite_received", "invite_200ok", "invite_4xx", "invite_5xx",
        "bye_received", "cancel_received", "ack_received",
        "options_received", "register_received",
        "dispatcher_active_destinations", "dispatcher_inactive_destinations",
        "trunks_online", "trunks_total",
        "active_transactions", "shm_used_mb", "shm_free_mb",
        "parse_errors", "script_errors", "db_errors",
    ],
    "rtpengine": [
        "active_sessions", "active_calls", "one_sided_sessions",
        "ports_used", "port_usage_percent",
        "packets_in", "packets_out", "bytes_in", "bytes_out",
        "packets_lost_in", "packets_lost_out",
        "jitter_avg_ms", "rtt_avg_ms", "mos_avg",
        "timeout_errors", "cpu_percent", "memory_mb",
    ],
    "mariadb": [
        "threads_connected", "threads_running",
        "questions", "slow_queries", "queries_per_second",
        "innodb_buffer_pool_used_mb", "innodb_buffer_pool_free_mb",
        "innodb_buffer_pool_pages_dirty", "innodb_deadlocks",
        "created_tmp_disk_tables", "created_tmp_tables",
    ],
    "system": [
        "cpu_percent", "load_avg_1m", "load_avg_5m", "load_avg_15m",
        "memory_percent", "swap_percent", "disk_percent",
        "disk_read_mbps", "disk_write_mbps",
        "net_rx_mbps", "net_tx_mbps", "net_rx_pps", "net_tx_pps",
        "kamailio_cpu_percent", "kamailio_memory_mb",
        "rtpengine_cpu_percent", "rtpengine_memory_mb",
        "freeswitch_cpu_percent", "freeswitch_memory_mb",
        "postgres_cpu_percent", "postgres_memory_mb",
    ],
}

ROLLUP_MODELS = {
    "kamailio": KamailioMetrics,
    "rtpengine": RtpengineMetrics,
    "mariadb": MariaDBMetrics,
    "system": SystemMetrics,
}

@dataclass
class CollectorResult:
    success: bool
    data: Dict[str, Any]
    error: Optional[str] = None


def parse_kamcmd_output(output: str) -> Dict[str, Any]:
    """Parse kamcmd output in key=value format"""
    result = {}
    for line in output.strip().split('\n'):
        line = line.strip()
        if '=' in line and not line.startswith('kamcmd>'):
            try:
                key, value = line.split('=', 1)
                key = key.strip()
                value = value.strip()
                # Try to convert to number
                try:
                    if '.' in value:
                        value = float(value)
                    else:
                        value = int(value)
                except:
                    pass
                result[key] = value
            except:
                pass
    return result


class KamailioCollector:
    """Collects metrics from Kamailio via kamcmd"""
    
    def __init__(self):
        self.kamcmd_path = "/usr/sbin/kamcmd"
    
    def _run_kamcmd(self, command: str) -> Optional[str]:
        try:
            parts = [self.kamcmd_path] + shlex.split(command)
            result = subprocess.run(
                parts,
                capture_output=True, text=True, timeout=5
            )
            if result.returncode == 0:
                return result.stdout
        except Exception as e:
            print(f"Kamailio collector error for {command}: {e}")
        return None
    
    def _run_kamcmd_json(self, command: str) -> Optional[Dict]:
        """Run kamcmd and parse key:value format output"""
        try:
            parts = [self.kamcmd_path] + shlex.split(command)
            result = subprocess.run(
                parts,
                capture_output=True, text=True, timeout=5
            )
            if result.returncode == 0:
                output = result.stdout.strip()
                # Parse key:value format (not valid JSON)
                result = {}
                for line in output.strip().split('\n'):
                    line = line.strip()
                    if ':' in line and not line.startswith('{') and not line.startswith('}') and not line.startswith('kamcmd>'):
                        try:
                            key, value = line.split(':', 1)
                            key = key.strip().strip('"')
                            value = value.strip().strip(',')
                            # Convert to number if possible
                            try:
                                if '.' in value:
                                    value = float(value)
                                else:
                                    value = int(value)
                            except:
                                pass
                            result[key] = value
                        except:
                            pass
                return result if result else None
        except Exception as e:
            print(f"Kamailio JSON collector error for {command}: {e}")
        return None
    
    # ---- Trunk health -------------------------------------------------------
    # SIP OPTIONS to every enabled routing_trunks row, sent BY Kamailio from its
    # own socket (tm.t_uac_wait_block), so the peer sees the same IP:port as the
    # real traffic and its ACL applies: public trunks from the carrier-facing
    # listener, private trunks from the LAN listener. Results update
    # routing_trunks.sip_status (shown in Enrutamiento) and routing_sip_status_log.

    def _kamailio_sockets(self) -> Dict[str, str]:
        defs = {}
        try:
            with open(KAMAILIO_CFG) as f:
                for line in f:
                    m = re.match(r'#!substdef "!(\w+)!([^!]*)!g"', line)
                    if m:
                        defs[m.group(1)] = m.group(2)
        except OSError:
            pass

        def sock(ip_key, port_key, env):
            if os.environ.get(env):
                return os.environ[env]
            if defs.get(ip_key) and defs.get(port_key):
                return f"udp:{defs[ip_key]}:{defs[port_key]}"
            return "."  # let Kamailio pick the socket

        return {
            "public": sock("EXTERNAL_IP_ADDR", "SIP_PORT", "KAM_PUBLIC_SOCKET"),
            "private": sock("INTERNAL_IP_ADDR", "FS_PORT", "KAM_PRIVATE_SOCKET"),
        }

    def _probe_trunk(self, trunk: Dict[str, Any], sockets: Dict[str, str]) -> Dict[str, Any]:
        result = {
            "id": str(trunk["id"]), "name": trunk["name"],
            "trunk_type": trunk["trunk_type"], "host": trunk["host"], "port": trunk["port"],
            "sip_status": "unreachable", "response_code": None, "latency_ms": None,
            "message": None, "log_status": "timeout",
        }
        transport = (trunk.get("transport") or "udp").lower()
        if transport != "udp":
            result.update(sip_status="unknown", message=f"Transporte {transport} no soportado por la sonda",
                          log_status="error")
            return result

        socket_ = sockets.get(trunk["trunk_type"], ".")
        src_host = socket_.split(":")[1] if socket_.count(":") == 2 else "127.0.0.1"
        target = f"{trunk['host']}:{trunk['port']}"
        headers = (
            f"From: <sip:apolo-monitor@{src_host}>;tag={uuid.uuid4().hex[:12]}\r\n"
            f"To: <sip:{target}>\r\n"
            "Max-Forwards: 70\r\n"
            "Content-Type: text/plain\r\n"  # tm RPC refuses a request without it
        )
        cmd = [self.kamcmd_path, "tm.t_uac_wait_block", "OPTIONS", f"sip:{target}", ".", socket_, headers]
        started = time.monotonic()
        try:
            out = subprocess.run(cmd, capture_output=True, text=True, timeout=TRUNK_PROBE_TIMEOUT)
        except subprocess.TimeoutExpired:
            result["message"] = f"Sin respuesta a OPTIONS en {TRUNK_PROBE_TIMEOUT}s"
            return result
        latency = int((time.monotonic() - started) * 1000)

        code = re.search(r"code:\s*(\d+)", out.stdout or "")
        reason = re.search(r"text:\s*(.+)", out.stdout or "")
        if not code:
            err = (out.stderr or out.stdout or "").strip()[:200]
            result.update(message=f"Kamailio no pudo enviar OPTIONS: {err}", log_status="error")
            return result

        code = int(code.group(1))
        reason = reason.group(1).strip() if reason else ""
        result.update(response_code=code, latency_ms=latency)
        # Any reply proves the peer is up; 408 is our own tm timeout and 5xx
        # means it can't take calls (503 overloaded/maintenance).
        if code < 500 and code != 408:
            result.update(sip_status="reachable", log_status="success",
                          message=f"{code} {reason} - Latencia: {latency}ms")
        else:
            result.update(log_status="timeout" if code == 408 else "error",
                          message=f"{code} {reason}")
        return result

    def _probe_trunks(self) -> Optional[List[Dict[str, Any]]]:
        db = SessionLocal()
        try:
            rows = db.execute(sa_text(
                "SELECT id, name, COALESCE(trunk_type, 'public') AS trunk_type, host, "
                "COALESCE(port, 5060) AS port, transport "
                "FROM routing_trunks WHERE enabled = true ORDER BY name"
            )).mappings().all()
            if not rows:
                return []
            sockets = self._kamailio_sockets()
            with ThreadPoolExecutor(max_workers=min(8, len(rows))) as pool:
                results = list(pool.map(lambda r: self._probe_trunk(dict(r), sockets), rows))

            for r in results:
                db.execute(sa_text(
                    "UPDATE routing_trunks SET sip_status = :st, sip_status_message = :msg, "
                    "last_options_check = NOW(), last_options_latency_ms = :lat, "
                    "last_options_response_code = :code WHERE id = :id"
                ), {"st": r["sip_status"], "msg": r["message"], "lat": r["latency_ms"],
                    "code": r["response_code"], "id": r["id"]})
                db.execute(sa_text(
                    "INSERT INTO routing_sip_status_log "
                    "(trunk_id, check_source, status, response_code, latency_ms, error_message) "
                    "VALUES (:id, 'kamailio', :st, :code, :lat, :err)"
                ), {"id": r["id"], "st": r["log_status"], "code": r["response_code"],
                    "lat": r["latency_ms"],
                    "err": None if r["sip_status"] == "reachable" else r["message"]})
            db.execute(sa_text(
                "DELETE FROM routing_sip_status_log "
                f"WHERE check_timestamp < NOW() - INTERVAL '{TRUNK_LOG_RETENTION_DAYS} days'"
            ))
            db.commit()
            return [{k: v for k, v in r.items() if k != "log_status"} for r in results]
        except Exception as e:
            db.rollback()
            print(f"Trunk probe failed: {e}")
            return None
        finally:
            db.close()

    def collect(self) -> CollectorResult:
        data = {}
        try:
            # Core stats
            core = self._run_kamcmd("stats.get_statistics all")
            if core:
                data["core"] = parse_kamcmd_output(core)
            
            # Dialog stats
            dialog = self._run_kamcmd_json("dlg.stats_active")
            if dialog:
                data["dialog"] = dialog
            
            # Trunk health (replaces dispatcher.list: the dispatcher is a dSIPRouter
            # leftover no call uses; routing is drouting to routing_trunks)
            trunks = self._probe_trunks()
            if trunks is not None:
                data["trunks"] = trunks
            
            # Transaction stats (tm.stats en formato clave:valor)
            tm = self._run_kamcmd_json("tm.stats")
            if tm:
                data["tm"] = tm
            
            # Shared memory
            shm = self._run_kamcmd_json("shm.stats")
            if shm:
                data["shm"] = shm
            
            # Parse into metrics
            metrics = self._parse_metrics(data)
            return CollectorResult(success=True, data=metrics, error=None)
            
        except Exception as e:
            print(f"Kamailio collection failed: {e}")
            return CollectorResult(success=False, data={}, error=str(e))
    
    def _parse_metrics(self, raw: Dict) -> Dict[str, Any]:
        metrics = {
            "timestamp": datetime.utcnow(),
            "raw_data": raw
        }
        
        # Dialog stats
        if "dialog" in raw:
            d = raw["dialog"]
            metrics["active_dialogs"] = d.get("all", 0)
            metrics["early_dialogs"] = d.get("connecting", 0) + d.get("answering", 0)
            metrics["confirmed_dialogs"] = d.get("ongoing", 0)
            metrics["active_calls"] = d.get("ongoing", 0)
        
        # Core stats
        if "core" in raw:
            c = raw["core"]
            metrics["invite_received"] = c.get("core:rcv_requests_invite", 0)
            metrics["bye_received"] = c.get("core:rcv_requests_bye", 0)
            metrics["cancel_received"] = c.get("core:rcv_requests_cancel", 0)
            metrics["ack_received"] = c.get("core:rcv_requests_ack", 0)
            metrics["options_received"] = c.get("core:rcv_requests_options", 0)
            metrics["register_received"] = c.get("core:rcv_requests_register", 0)
            metrics["invite_200ok"] = c.get("core:rcv_replies_2xx_invite", 0)
            metrics["invite_4xx"] = c.get("core:rcv_replies_4xx_invite", 0)
            metrics["invite_5xx"] = c.get("core:rcv_replies_5xx_invite", 0)
            metrics["parse_errors"] = c.get("core:err_requests", 0) + c.get("core:err_replies", 0)
        
        # TM stats (tm.stats -> {"current": N, ...}; legacy tmx:active_transactions)
        if "tm" in raw:
            t = raw["tm"]
            metrics["active_transactions"] = t.get("current", t.get("tmx:active_transactions", 0))
            metrics["transaction_timeouts"] = 0  # Not directly available
        
        # Shared memory
        if "shm" in raw:
            s = raw["shm"]
            metrics["shm_used_mb"] = s.get("real_used", 0) / (1024 * 1024)
            metrics["shm_free_mb"] = s.get("free", 0) / (1024 * 1024)
            metrics["shm_fragments"] = s.get("fragments", 0)
            metrics["pkg_used_mb"] = 0
        
        # Trunk health (see _probe_trunks)
        if "trunks" in raw:
            metrics["trunks_total"] = len(raw["trunks"])
            metrics["trunks_online"] = sum(1 for t in raw["trunks"] if t["sip_status"] == "reachable")

        return metrics


def _bencode(o) -> bytes:
    if isinstance(o, bytes):
        return str(len(o)).encode() + b':' + o
    if isinstance(o, str):
        b = o.encode()
        return str(len(b)).encode() + b':' + b
    if isinstance(o, int):
        return b'i' + str(o).encode() + b'e'
    if isinstance(o, list):
        return b'l' + b''.join(_bencode(x) for x in o) + b'e'
    if isinstance(o, dict):
        out = b'd'
        for k in sorted(o.keys(), key=lambda x: x if isinstance(x, bytes) else x.encode()):
            out += _bencode(k) + _bencode(o[k])
        return out + b'e'
    raise TypeError(f"cannot bencode {type(o)}")


def _bdecode(data: bytes):
    def parse(i: int):
        c = data[i:i + 1]
        if c == b'i':
            j = data.index(b'e', i)
            return int(data[i + 1:j]), j + 1
        if c == b'l':
            i += 1
            out = []
            while data[i:i + 1] != b'e':
                v, i = parse(i)
                out.append(v)
            return out, i + 1
        if c == b'd':
            i += 1
            out = {}
            while data[i:i + 1] != b'e':
                k, i = parse(i)
                v, i = parse(i)
                out[k] = v
            return out, i + 1
        if c.isdigit():
            j = data.index(b':', i)
            n = int(data[i:j])
            s = data[j + 1:j + 1 + n]
            return s, j + 1 + n
        raise ValueError(f"bad bencode at {i}: {data[i:i+20]!r}")
    v, _ = parse(0)
    return v


def _clean_ng(o):
    if isinstance(o, bytes):
        try:
            return o.decode()
        except Exception:
            return repr(o[:80])
    if isinstance(o, dict):
        return {_clean_ng(k): _clean_ng(v) for k, v in o.items()}
    if isinstance(o, list):
        return [_clean_ng(x) for x in o]
    return o


class RtpengineCollector:
    """Collects REAL media metrics from RTPengine via NG protocol (UDP 7722).

    Sources (in priority order):
    - NG `list`        -> authoritative active call-ids = active_sessions
    - NG `statistics`  -> sessionsown/total, media streams, packet/byte rates,
                          relayed totals, timeouts, avg call duration, MOS aggregates,
                          voip_metrics (jitter/RTT/loss), offer/answer/delete counts
    - NG `query` (sampled, max 8 calls) -> per-call packets/bytes, ports, codec,
                          one-sided detection
    - kamcmd rtpengine.get_hash_total -> fallback session count (Kamailio view)
    - psutil -> process cpu/mem/threads
    """

    def __init__(self):
        self.ng_host = "127.0.0.1"
        self.ng_port = 7722
        self.kamcmd_path = "/usr/sbin/kamcmd"
        self.port_min, self.port_max = self._read_port_range()
        self.query_sample = 8
        self.direction_check_cap = 500

    @staticmethod
    def _read_port_range(path: str = "/etc/rtpengine/rtpengine.conf") -> tuple:
        """Read port-min/port-max from the live rtpengine config (defaults 10000-20000)."""
        pmin, pmax = 10000, 20000
        try:
            with open(path) as f:
                for line in f:
                    m = re.match(r'\s*port-(min|max)\s*=\s*(\d+)', line)
                    if m:
                        if m.group(1) == "min":
                            pmin = int(m.group(2))
                        else:
                            pmax = int(m.group(2))
        except Exception as e:
            print(f"RTPengine config read error: {e}")
        return pmin, pmax

    def _ng(self, payload: Dict, timeout: float = 3.0) -> Optional[Any]:
        try:
            cookie = uuid.uuid4().hex[:8]
            packet = cookie.encode() + b' ' + _bencode(payload)
            s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            s.settimeout(timeout)
            try:
                s.sendto(packet, (self.ng_host, self.ng_port))
                data, _ = s.recvfrom(262144)
            finally:
                s.close()
            sp = data.find(b' ')
            body = data[sp + 1:] if sp != -1 else data
            return _bdecode(body)
        except Exception as e:
            print(f"RTPengine NG error for {payload.get('command')}: {e}")
            return None

    def _kamcmd(self, command: str) -> Optional[str]:
        try:
            r = subprocess.run([self.kamcmd_path, command], capture_output=True, text=True, timeout=5)
            if r.returncode == 0:
                return r.stdout
        except Exception as e:
            print(f"RTPengine kamcmd fallback error for {command}: {e}")
        return None
    
    def collect(self) -> CollectorResult:
        data: Dict[str, Any] = {}
        try:
            # 1) Process info (always available)
            for proc in psutil.process_iter(['pid', 'name', 'cpu_percent', 'memory_info', 'num_threads']):
                try:
                    if proc.info['name'] and 'rtpengine' in proc.info['name'].lower():
                        data["process"] = {
                            "cpu_percent": proc.info['cpu_percent'],
                            "memory_mb": proc.info['memory_info'].rss / (1024 * 1024),
                            "threads": proc.info['num_threads'],
                        }
                        break
                except Exception:
                    pass

            # 2) NG list -> active call-ids. rtpengine caps `list` at 32 call-ids
            #    unless `limit` is given, which pegged the chart at 32 under load.
            ng_list = self._ng({"command": "list", "limit": 100000})
            callids: List[str] = []
            if isinstance(ng_list, dict):
                raw_calls = ng_list.get(b'calls', ng_list.get('calls', []))
                if isinstance(raw_calls, list):
                    for c in raw_calls:
                        callids.append(c.decode() if isinstance(c, bytes) else str(c))
            data["ng_list_count"] = len(callids)
            data["ng_callids_sample"] = callids[:5]

            # 3) NG statistics -> global media KPIs
            ng_stats = self._ng({"command": "statistics"})
            if isinstance(ng_stats, dict):
                data["ng_statistics"] = _clean_ng(ng_stats)

            # 4) Per-call query. Ports/packets come from a small sample; media
            #    direction is checked on every session (bounded) because a
            #    sample of 8 extrapolated to the total swung 0..20 per minute.
            #    A session counts as one-sided only once it is past ringing
            #    (older than ONE_SIDED_MIN_AGE): before answer, no media or
            #    callee-only early media is normal.
            sampled_packets = 0
            sampled_bytes = 0
            sampled_ports: set = set()
            one_sided = 0
            no_media = 0
            queried = 0
            sampled = 0
            now_ts = time.time()
            for idx, cid in enumerate(callids[:self.direction_check_cap]):
                q = self._ng({"command": "query", "call-id": cid}, timeout=3.0)
                if not isinstance(q, dict):
                    continue
                queried += 1
                in_sample = idx < self.query_sample
                if in_sample:
                    sampled += 1
                try:
                    tags = q.get(b'tags', q.get('tags', {}))
                    if not isinstance(tags, dict):
                        continue
                    pkts_per_tag = []
                    for _tag, tinfo in tags.items():
                        if not isinstance(tinfo, dict):
                            continue
                        tag_pkts = 0
                        medias = tinfo.get(b'medias', tinfo.get('medias', []))
                        if not isinstance(medias, list):
                            continue
                        for m in medias:
                            if not isinstance(m, dict):
                                continue
                            streams = m.get(b'streams', m.get('streams', []))
                            if not isinstance(streams, list):
                                continue
                            for st in streams:
                                if not isinstance(st, dict):
                                    continue
                                stats = st.get(b'stats', st.get('stats', {}))
                                pk = stats.get(b'packets', stats.get('packets', 0)) if isinstance(stats, dict) else 0
                                if isinstance(pk, int):
                                    tag_pkts += pk
                                if not in_sample:
                                    continue
                                lp = st.get(b'local port', st.get('local port'))
                                if isinstance(lp, int):
                                    sampled_ports.add(lp)
                                by = stats.get(b'bytes', stats.get('bytes', 0)) if isinstance(stats, dict) else 0
                                if isinstance(pk, int):
                                    sampled_packets += pk
                                if isinstance(by, int):
                                    sampled_bytes += by
                        pkts_per_tag.append(tag_pkts)
                    created = q.get(b'created', q.get('created', 0))
                    age = now_ts - created if isinstance(created, (int, float)) and created > 0 else 0
                    if age < ONE_SIDED_MIN_AGE or not pkts_per_tag:
                        continue
                    with_media = sum(1 for p in pkts_per_tag if p > 0)
                    if with_media == 0:
                        no_media += 1
                    elif with_media < 2:
                        one_sided += 1
                except Exception:
                    continue
            data["ng_queried"] = sampled  # sample size behind ports/packets
            data["ng_direction_checked"] = queried
            data["ng_sampled_packets"] = sampled_packets
            data["ng_sampled_bytes"] = sampled_bytes
            data["ng_sampled_ports"] = sorted(sampled_ports)
            data["ng_one_sided"] = one_sided
            data["ng_no_media"] = no_media

            # 5) Kamailio view fallback (hash table entries using rtpengine)
            hash_total = None
            out = self._kamcmd("rtpengine.get_hash_total")
            if out:
                m = re.search(r'(\d+)', out)
                if m:
                    hash_total = int(m.group(1))
            data["kam_hash_total"] = hash_total

            metrics = self._parse_metrics(data)
            return CollectorResult(success=True, data=metrics, error=None)

        except Exception as e:
            print(f"RTPengine collection failed: {e}")
            return CollectorResult(success=False, data={}, error=str(e))
    
    def _parse_metrics(self, raw: Dict) -> Dict[str, Any]:
        def _f(x, d=0.0):
            try:
                return float(x)
            except Exception:
                return d

        def _i(x, d=0):
            try:
                return int(float(x))
            except Exception:
                return d

        metrics: Dict[str, Any] = {
            "timestamp": datetime.utcnow(),
            "raw_data": {k: v for k, v in raw.items() if k not in ("ng_statistics",)},
        }

        if "process" in raw:
            p = raw["process"]
            metrics["cpu_percent"] = p.get("cpu_percent", 0) or 0.0
            metrics["memory_mb"] = p.get("memory_mb", 0) or 0.0
            metrics["threads"] = p.get("threads", 0) or 0

        stats = (raw.get("ng_statistics") or {})
        cur = stats.get("statistics", {}).get("currentstatistics", {}) if isinstance(stats.get("statistics"), dict) else {}
        tot = stats.get("statistics", {}).get("totalstatistics", {}) if isinstance(stats.get("statistics"), dict) else {}
        mos = stats.get("statistics", {}).get("mos", {}) if isinstance(stats.get("statistics"), dict) else {}
        voip = stats.get("statistics", {}).get("voip_metrics", {}) if isinstance(stats.get("statistics"), dict) else {}
        ctrl = stats.get("statistics", {}).get("controlstatistics", {}) if isinstance(stats.get("statistics"), dict) else {}

        # Active sessions: statistics.sessionstotal is uncapped; fallback to NG list
        # count, then kamailio hash
        ng_count = raw.get("ng_list_count") or 0
        sess_total = _i(cur.get("sessionstotal", 0))
        kam_hash = raw.get("kam_hash_total")
        active_sessions = sess_total or ng_count or (kam_hash or 0)
        metrics["active_sessions"] = int(active_sessions)
        metrics["active_calls"] = int(active_sessions)

        # Sessions past ringing with media missing in one or both directions
        # (one-way audio, dead media, or a call on hold). Counted on every
        # session, not extrapolated.
        metrics["one_sided_sessions"] = int((raw.get("ng_one_sided") or 0) + (raw.get("ng_no_media") or 0))
        metrics["deleted_sessions"] = 0

        # Ports: each media stream uses RTP+RTCP (2 ports). media_userspace = streams.
        media_streams = _i(cur.get("media_userspace", 0))
        ports_used = media_streams * 2
        # Cross-check with sampled local ports scaled to all sessions
        sampled_ports = raw.get("ng_sampled_ports") or []
        queried = raw.get("ng_queried") or 0
        if queried > 0 and active_sessions > 0 and sampled_ports:
            avg_ports = len(sampled_ports) / queried
            ports_used = int(round(avg_ports * active_sessions))
        port_range = max(1, self.port_max - self.port_min + 1)
        metrics["ports_used"] = int(ports_used)
        metrics["ports_free"] = max(0, port_range - int(ports_used))
        metrics["port_usage_percent"] = round(min(100.0, int(ports_used) / port_range * 100), 2)

        # Traffic: cumulative relayed totals (authoritative), rates instantaneous
        metrics["packets_in"] = _i(tot.get("relayedpackets", tot.get("relayedpackets_user", 0)))
        metrics["packets_out"] = _i(tot.get("relayedpackets", tot.get("relayedpackets_user", 0)))
        metrics["bytes_in"] = _i(tot.get("relayedbytes", tot.get("relayedbytes_user", 0)))
        metrics["bytes_out"] = _i(tot.get("relayedbytes", tot.get("relayedbytes_user", 0)))
        metrics["packets_lost_in"] = _i(voip.get("packets_lost", 0))
        metrics["packets_lost_out"] = _i(voip.get("packets_lost", 0))
        metrics["jitter_avg_ms"] = _f(voip.get("jitter_average", 0.0))
        metrics["rtt_avg_ms"] = _f(voip.get("rtt_e2e_average", voip.get("rtt_dsct_average", 0.0)))
        metrics["mos_avg"] = _f(mos.get("mos_average", 0.0))
        metrics["offer_errors"] = 0
        metrics["answer_errors"] = 0
        metrics["delete_errors"] = 0
        metrics["timeout_errors"] = _i(tot.get("timeoutsessions", 0)) + _i(tot.get("offertimeoutsessions", 0))

        # Keep raw NG stats (cleaned) for detail drawer + debugging
        metrics["raw_data"]["ng_currentstatistics"] = cur
        metrics["raw_data"]["ng_totalstatistics"] = {
            k: tot.get(k) for k in (
                "managedsessions", "timeoutsessions", "silenttimeoutsessions",
                "offertimeoutsessions", "regularterminatedsessions",
                "relayedpackets", "relayedbytes", "avgcallduration",
                "onewaystreams", "zerowaystreams",
            ) if k in tot
        }
        # *_average are lifetime averages since RTPEngine started; the running
        # sums + sample counts let the dashboard average over its own window
        # (and see when no RTCP sample arrived at all, e.g. kernel forwarding)
        metrics["raw_data"]["ng_mos"] = {
            k: mos.get(k) for k in ("mos_average", "mos_total", "mos_samples_total") if k in mos
        }
        metrics["raw_data"]["ng_voip"] = {
            k: voip.get(k) for k in (
                "jitter_average", "jitter_total", "jitter_samples_total", "rtt_e2e_average",
                "rtt_dsct_average", "packetloss_average", "packetloss_total",
                "packetloss_samples_total", "packets_lost",
            ) if k in voip
        }
        proxies = ctrl.get("proxies", []) if isinstance(ctrl, dict) else []
        if proxies and isinstance(proxies, list) and isinstance(proxies[0], dict):
            px = proxies[0]
            metrics["raw_data"]["ng_control"] = {
                k: px.get(k) for k in ("offercount", "answercount", "deletecount", "querycount", "listcount") if k in px
            }
            metrics["offer_errors"] = 0
            metrics["answer_errors"] = 0
            metrics["delete_errors"] = 0

        return metrics


class MariaDBCollector:
    """Collects metrics from MariaDB/MySQL"""
    
    def __init__(self):
        self.config = {
            'host': 'localhost',
            'port': 3306,
            'user': 'apolo',  # Correct user
            'password': 'ApoloNext.2026',
            'database': 'kamailio',
            'connection_timeout': 3
        }
        # Previous (Questions, Uptime) sample: QPS is their delta, not the
        # running total since MariaDB started
        self.last_questions = None
    
    def collect(self) -> CollectorResult:
        try:
            conn = mysql.connector.connect(**self.config)
            cursor = conn.cursor(dictionary=True)
            
            data = {}
            
            # Global status
            cursor.execute("SHOW GLOBAL STATUS")
            status = {row['Variable_name']: row['Value'] for row in cursor.fetchall()}
            data["status"] = status
            
            # Global variables
            cursor.execute("SHOW GLOBAL VARIABLES")
            variables = {row['Variable_name']: row['Value'] for row in cursor.fetchall()}
            data["variables"] = variables
            
            # InnoDB status
            cursor.execute("SHOW ENGINE INNODB STATUS")
            innodb = cursor.fetchone()
            data["innodb_status"] = innodb['Status'] if innodb else ""
            
            # Process list
            cursor.execute("SHOW PROCESSLIST")
            processes = cursor.fetchall()
            data["processes"] = processes
            
            cursor.close()
            conn.close()
            
            metrics = self._parse_metrics(data)
            return CollectorResult(success=True, data=metrics, error=None)
            
        except MySQLError as e:
            print(f"MariaDB collection failed: {e}")
            return CollectorResult(success=False, data={}, error=str(e))
        except Exception as e:
            print(f"MariaDB collection error: {e}")
            return CollectorResult(success=False, data={}, error=str(e))
    
    def _qps(self, questions: int, uptime: int) -> Optional[float]:
        """Questions/s since the previous sample; None on the first sample or
        after a MariaDB restart (counter reset)"""
        prev, self.last_questions = self.last_questions, (questions, uptime)
        if not prev or uptime <= prev[1] or questions < prev[0]:
            return None
        return (questions - prev[0]) / (uptime - prev[1])

    def _parse_metrics(self, raw: Dict) -> Dict[str, Any]:
        s = raw.get("status", {})
        v = raw.get("variables", {})
        
        def get_int(key: str, default: int = 0) -> int:
            try:
                return int(s.get(key, default))
            except:
                return default
        
        def get_float(key: str, default: float = 0.0) -> float:
            try:
                return float(s.get(key, default))
            except:
                return default
        
        def get_var_int(key: str, default: int = 0) -> int:
            try:
                return int(v.get(key, default))
            except:
                return default
        
        # InnoDB buffer pool calculations
        page_size = get_int("Innodb_page_size", 16384)
        pages_data = get_int("Innodb_buffer_pool_pages_data")
        pages_free = get_int("Innodb_buffer_pool_pages_free")
        
        # Get actual max_connections from variables (not max_used_connections)
        max_connections = get_var_int("max_connections", 151)
        
        metrics = {
            "timestamp": datetime.utcnow(),
            "raw_data": raw,
            
            "threads_connected": get_int("Threads_connected"),
            "threads_running": get_int("Threads_running"),
            "max_used_connections": get_int("Max_used_connections"),
            "aborted_connects": get_int("Aborted_connects"),
            "connection_errors": get_int("Connection_errors_total"),
            "questions": get_int("Questions"),
            "slow_queries": get_int("Slow_queries"),
            "queries_per_second": self._qps(get_int("Questions"), get_int("Uptime")),
            "innodb_buffer_pool_used_mb": pages_data * page_size / (1024 * 1024),
            "innodb_buffer_pool_free_mb": pages_free * page_size / (1024 * 1024),
            "innodb_buffer_pool_pages_dirty": get_int("Innodb_buffer_pool_pages_dirty"),
            "innodb_row_lock_waits": get_int("Innodb_row_lock_waits"),
            "innodb_deadlocks": get_int("Innodb_deadlocks"),
            "slave_sql_running": s.get("Slave_SQL_Running") == "Yes" if "Slave_SQL_Running" in s else None,
            "slave_io_running": s.get("Slave_IO_Running") == "Yes" if "Slave_IO_Running" in s else None,
            "seconds_behind_master": get_int("Seconds_Behind_Master") if "Seconds_Behind_Master" in s else None,
            "created_tmp_disk_tables": get_int("Created_tmp_disk_tables"),
            "created_tmp_tables": get_int("Created_tmp_tables"),
            # Override max_used_connections with actual max_connections for threshold calculation
            "max_used_connections": max_connections,
        }
        
        return metrics


class SystemCollector:
    """Collects system-level metrics using psutil"""
    
    def __init__(self):
        self.last_net_io = None
        self.last_disk_io = None
        self.last_time = None
    
    def collect(self) -> CollectorResult:
        try:
            now = time.time()
            metrics = {
                "timestamp": datetime.utcnow(),
                "hostname": socket.gethostname()
            }
            
            # CPU
            cpu_percent = psutil.cpu_percent(interval=0.5)
            cpu_times = psutil.cpu_times_percent(interval=0.1)
            load_avg = psutil.getloadavg()
            
            metrics["cpu_percent"] = cpu_percent
            metrics["cpu_user_percent"] = cpu_times.user
            metrics["cpu_system_percent"] = cpu_times.system
            metrics["cpu_iowait_percent"] = getattr(cpu_times, 'iowait', 0)
            metrics["load_avg_1m"] = load_avg[0]
            metrics["load_avg_5m"] = load_avg[1]
            metrics["load_avg_15m"] = load_avg[2]
            metrics["cpu_cores"] = psutil.cpu_count()
            
            # Memory
            mem = psutil.virtual_memory()
            swap = psutil.swap_memory()
            
            metrics["memory_total_mb"] = mem.total / (1024 * 1024)
            metrics["memory_used_mb"] = mem.used / (1024 * 1024)
            metrics["memory_free_mb"] = mem.free / (1024 * 1024)
            metrics["memory_available_mb"] = mem.available / (1024 * 1024)
            metrics["memory_percent"] = mem.percent
            metrics["swap_total_mb"] = swap.total / (1024 * 1024)
            metrics["swap_used_mb"] = swap.used / (1024 * 1024)
            metrics["swap_percent"] = swap.percent
            
            # Disk
            disk = psutil.disk_usage('/')
            metrics["disk_total_gb"] = disk.total / (1024**3)
            metrics["disk_used_gb"] = disk.used / (1024**3)
            metrics["disk_free_gb"] = disk.free / (1024**3)
            metrics["disk_percent"] = (disk.used / disk.total) * 100
            
            # Disk I/O rate
            disk_io = psutil.disk_io_counters()
            if self.last_disk_io and self.last_time:
                dt = now - self.last_time
                metrics["disk_read_mbps"] = (disk_io.read_bytes - self.last_disk_io.read_bytes) / dt / (1024*1024)
                metrics["disk_write_mbps"] = (disk_io.write_bytes - self.last_disk_io.write_bytes) / dt / (1024*1024)
                metrics["disk_iops_read"] = int((disk_io.read_count - self.last_disk_io.read_count) / dt)
                metrics["disk_iops_write"] = int((disk_io.write_count - self.last_disk_io.write_count) / dt)
            self.last_disk_io = disk_io
            
            # Network
            net_io = psutil.net_io_counters(pernic=True)
            primary_iface = "eth0"
            for iface, stats in net_io.items():
                if iface != "lo" and (stats.bytes_sent > 0 or stats.bytes_recv > 0):
                    primary_iface = iface
                    break
            
            metrics["net_interface"] = primary_iface
            if primary_iface in net_io:
                stats = net_io[primary_iface]
                if self.last_net_io and self.last_time:
                    dt = now - self.last_time
                    metrics["net_rx_mbps"] = (stats.bytes_recv - self.last_net_io.bytes_recv) / dt / (1024*1024)
                    metrics["net_tx_mbps"] = (stats.bytes_sent - self.last_net_io.bytes_sent) / dt / (1024*1024)
                    metrics["net_rx_pps"] = int((stats.packets_recv - self.last_net_io.packets_recv) / dt)
                    metrics["net_tx_pps"] = int((stats.packets_sent - self.last_net_io.packets_sent) / dt)
                metrics["net_errors"] = stats.errin + stats.errout
                metrics["net_drops"] = stats.dropin + stats.dropout
            self.last_net_io = net_io.get(primary_iface) if primary_iface in net_io else None
            
            # Process-specific metrics
            for proc in psutil.process_iter(['pid', 'name', 'cpu_percent', 'memory_info', 'num_threads']):
                try:
                    name = proc.info['name'] or ''
                    if 'kamailio' in name.lower():
                        metrics["kamailio_cpu_percent"] = proc.info['cpu_percent']
                        metrics["kamailio_memory_mb"] = proc.info['memory_info'].rss / (1024*1024)
                        metrics["kamailio_threads"] = proc.info['num_threads']
                    elif 'rtpengine' in name.lower():
                        metrics["rtpengine_cpu_percent"] = proc.info['cpu_percent']
                        metrics["rtpengine_memory_mb"] = proc.info['memory_info'].rss / (1024*1024)
                    elif 'freeswitch' in name.lower():
                        metrics["freeswitch_cpu_percent"] = proc.info['cpu_percent']
                        metrics["freeswitch_memory_mb"] = proc.info['memory_info'].rss / (1024*1024)
                    elif 'postgres' in name.lower() or 'postmaster' in name.lower():
                        metrics["postgres_cpu_percent"] = proc.info['cpu_percent']
                        metrics["postgres_memory_mb"] = proc.info['memory_info'].rss / (1024*1024)
                except:
                    pass
            
            self.last_time = now
            
            return CollectorResult(success=True, data=metrics, error=None)
            
        except Exception as e:
            print(f"System collection failed: {e}")
            return CollectorResult(success=False, data={}, error=str(e))


# Rest of the monitoring service remains the same

class MonitoringService:
    """Main monitoring orchestration service"""
    
    def __init__(self):
        self.kamailio = KamailioCollector()
        self.rtpengine = RtpengineCollector()
        self.mariadb = MariaDBCollector()
        self.system = SystemCollector()
        self.running = False
        self.interval = 60  # seconds
    
    async def collect_all(self) -> Dict[str, CollectorResult]:
        """Run all collectors in parallel"""
        results = {}
        
        # Run collectors concurrently
        tasks = {
            "kamailio": asyncio.to_thread(self.kamailio.collect),
            "rtpengine": asyncio.to_thread(self.rtpengine.collect),
            "mariadb": asyncio.to_thread(self.mariadb.collect),
            "system": asyncio.to_thread(self.system.collect),
        }
        
        for name, task in tasks.items():
            try:
                results[name] = await asyncio.wait_for(task, timeout=30)
            except asyncio.TimeoutError:
                results[name] = CollectorResult(success=False, data={}, error="Timeout")
            except Exception as e:
                results[name] = CollectorResult(success=False, data={}, error=str(e))
        
        return results
    
    def store_metrics(self, results: Dict[str, CollectorResult]):
        """Store collected metrics to database"""
        db = SessionLocal()
        try:
            if results.get("kamailio", CollectorResult(False, {})).success:
                km = results["kamailio"].data
                db.add(KamailioMetrics(**km))
            
            if results.get("rtpengine", CollectorResult(False, {})).success:
                rm = results["rtpengine"].data
                db.add(RtpengineMetrics(**rm))
            
            if results.get("mariadb", CollectorResult(False, {})).success:
                mm = results["mariadb"].data
                db.add(MariaDBMetrics(**mm))
            
            if results.get("system", CollectorResult(False, {})).success:
                sm = results["system"].data
                db.add(SystemMetrics(**sm))
            
            db.commit()

            # Check alerts after storing
            self._check_alerts(db)

            # Carrier-class history: hourly/daily rollups + retention (cheap, idempotent)
            try:
                self._rollup_hourly(db)
                self._rollup_daily(db)
            except Exception as e:
                print(f"Rollup failed (non-fatal): {e}")
                db.rollback()
            # Retention runs at most once per hour (gated by hour bucket marker)
            try:
                self._retention_gated(db)
            except Exception as e:
                print(f"Retention failed (non-fatal): {e}")
                db.rollback()

        except Exception as e:
            print(f"Failed to store metrics: {e}")
            db.rollback()
        finally:
            db.close()

    def _rollup_hourly(self, db, lookback_hours: int = 3):
        """Aggregate completed hour buckets into monitoring_kpi_hourly.

        Rolls the last `lookback_hours` completed hours (default 3) instead of
        just one, so restarts or missed cycles self-heal. Idempotent: rows for
        each bucket are deleted before insert, so re-runs never duplicate.
        """
        now = datetime.utcnow().replace(minute=0, second=0, microsecond=0)
        for h in range(1, lookback_hours + 1):
            bucket = now - timedelta(hours=h)
            bucket_end = bucket + timedelta(hours=1)
            self._rollup_hour_bucket(db, bucket, bucket_end)
        db.commit()

    def _rollup_hour_bucket(self, db, bucket, bucket_end):
        for component, model in ROLLUP_MODELS.items():
            cols = [c.key for c in model.__table__.columns]
            for metric in ROLLUP_METRICS.get(component, []):
                if metric not in cols:
                    continue
                col = getattr(model, metric)
                row = db.query(
                    sa_func.avg(col), sa_func.min(col),
                    sa_func.max(col), sa_func.count(col),
                ).filter(
                    model.timestamp >= bucket, model.timestamp < bucket_end,
                ).first()
                if not row or (row[3] or 0) == 0 or row[0] is None:
                    continue
                db.query(MonitoringKpiHourly).filter(
                    MonitoringKpiHourly.component == component,
                    MonitoringKpiHourly.metric == metric,
                    MonitoringKpiHourly.bucket_start == bucket,
                ).delete()
                db.add(MonitoringKpiHourly(
                    component=component, metric=metric, bucket_start=bucket,
                    avg_value=float(row[0]), min_value=float(row[1]),
                    max_value=float(row[2]), samples=int(row[3]),
                ))
        db.commit()

    def _rollup_daily(self, db):
        """Aggregate yesterday's hourly rollups into monitoring_kpi_daily.

        Aggregating hourly (not raw) keeps this O(24 rows/metric) forever.
        Idempotent per day bucket.
        """
        today = datetime.utcnow().replace(hour=0, minute=0, second=0, microsecond=0)
        bucket = today - timedelta(days=1)
        bucket_end = today
        pairs = db.query(
            MonitoringKpiHourly.component, MonitoringKpiHourly.metric,
        ).filter(
            MonitoringKpiHourly.bucket_start >= bucket,
            MonitoringKpiHourly.bucket_start < bucket_end,
        ).distinct().all()
        for component, metric in pairs:
            rows = db.query(MonitoringKpiHourly).filter(
                MonitoringKpiHourly.component == component,
                MonitoringKpiHourly.metric == metric,
                MonitoringKpiHourly.bucket_start >= bucket,
                MonitoringKpiHourly.bucket_start < bucket_end,
            ).all()
            if not rows:
                continue
            # Weighted average by samples; min of mins; max of maxes
            total_w = sum(r.samples for r in rows) or 1
            avg_v = sum(r.avg_value * r.samples for r in rows) / total_w
            db.query(MonitoringKpiDaily).filter(
                MonitoringKpiDaily.component == component,
                MonitoringKpiDaily.metric == metric,
                MonitoringKpiDaily.bucket_start == bucket,
            ).delete()
            db.add(MonitoringKpiDaily(
                component=component, metric=metric, bucket_start=bucket,
                avg_value=float(avg_v),
                min_value=float(min(r.min_value for r in rows)),
                max_value=float(max(r.max_value for r in rows)),
                samples=int(sum(r.samples for r in rows)),
            ))
        db.commit()

    _last_retention_run: Optional[datetime] = None

    def _retention_gated(self, db):
        """Run retention at most once per hour."""
        now = datetime.utcnow()
        if self._last_retention_run and (now - self._last_retention_run) < timedelta(hours=1):
            return
        self._retention(db)
        self._last_retention_run = now

    def _retention(self, db):
        """Carrier-class retention:
        - raw numeric rows: monitoring_config.retention_days per component (default 30)
        - raw_data JSONB stripped after 48h (heavy payload, forensics only)
        - hourly rollups: 93 days, daily rollups: 400 days
        """
        now = datetime.utcnow()
        configs = {c.component: c for c in db.query(MonitoringConfig).all()}

        def raw_days(component: str) -> int:
            try:
                return int((configs.get(component) or {}).retention_days if hasattr(configs.get(component), 'retention_days') else 30)
            except Exception:
                return 30

        for component, model in ROLLUP_MODELS.items():
            days = raw_days(component) if component in configs else 30
            cutoff = now - timedelta(days=days)
            db.query(model).filter(model.timestamp < cutoff).delete()
        # Strip heavy JSON payloads after 48h (keep numeric history)
        json_cutoff = now - timedelta(hours=RAW_JSON_STRIP_HOURS)
        for model in ROLLUP_MODELS.values():
            try:
                db.query(model).filter(
                    model.timestamp < json_cutoff,
                    model.raw_data.isnot(None),
                ).update({model.raw_data: None}, synchronize_session=False)
            except Exception:
                pass
        db.query(MonitoringKpiHourly).filter(
            MonitoringKpiHourly.bucket_start < now - timedelta(days=HOURLY_RETENTION_DAYS),
        ).delete()
        db.query(MonitoringKpiDaily).filter(
            MonitoringKpiDaily.bucket_start < now - timedelta(days=DAILY_RETENTION_DAYS),
        ).delete()
        # Resolved alerts older than 30d are pruned (audit trail, not KPIs)
        db.query(MonitoringAlert).filter(
            MonitoringAlert.status == "resolved",
            MonitoringAlert.timestamp < now - timedelta(days=30),
        ).delete()
        db.commit()
    
    def _check_alerts(self, db):
        """Evaluate alert rules against current metrics"""
        try:
            # Get active alert rules
            configs = db.query(MonitoringConfig).filter(MonitoringConfig.enabled == True).all()
            
            for config in configs:
                thresholds = config.thresholds
                component = config.component
                
                # Get latest metrics for component
                latest_metrics = None
                if component == "kamailio":
                    latest_metrics = db.query(KamailioMetrics).order_by(KamailioMetrics.timestamp.desc()).first()
                elif component == "rtpengine":
                    latest_metrics = db.query(RtpengineMetrics).order_by(RtpengineMetrics.timestamp.desc()).first()
                elif component == "mariadb":
                    latest_metrics = db.query(MariaDBMetrics).order_by(MariaDBMetrics.timestamp.desc()).first()
                elif component == "system":
                    latest_metrics = db.query(SystemMetrics).order_by(SystemMetrics.timestamp.desc()).first()
                
                if not latest_metrics:
                    continue
                
                # Check each threshold
                for rule_name, rule in thresholds.items():
                    metric_name = rule.get("metric")
                    threshold = rule.get("threshold")
                    operator = rule.get("operator", ">")
                    severity = rule.get("severity", "warning")
                    enabled = rule.get("enabled", True)
                    
                    if not enabled or metric_name is None or threshold is None:
                        continue
                    
                    current_value = getattr(latest_metrics, metric_name, None)
                    if current_value is None:
                        continue
                    
                    # Evaluate condition
                    triggered = False
                    if operator == ">" and current_value > threshold:
                        triggered = True
                    elif operator == "<" and current_value < threshold:
                        triggered = True
                    elif operator == ">=" and current_value >= threshold:
                        triggered = True
                    elif operator == "<=" and current_value <= threshold:
                        triggered = True
                    elif operator == "==" and current_value == threshold:
                        triggered = True
                    
                    if triggered:
                        # Check if alert already firing
                        existing = db.query(MonitoringAlert).filter(
                            MonitoringAlert.alert_name == rule_name,
                            MonitoringAlert.component == component,
                            MonitoringAlert.status == "firing"
                        ).first()
                        
                        if not existing:
                            alert = MonitoringAlert(
                                alert_name=rule_name,
                                severity=severity,
                                component=component,
                                metric_name=metric_name,
                                threshold_value=threshold,
                                current_value=current_value,
                                operator=operator,
                                status="firing",
                                message=f"{component}.{metric_name} {operator} {threshold} (current: {current_value})"
                            )
                            db.add(alert)
                    else:
                        # Resolve any firing alerts for this rule
                        db.query(MonitoringAlert).filter(
                            MonitoringAlert.alert_name == rule_name,
                            MonitoringAlert.component == component,
                            MonitoringAlert.status == "firing"
                        ).update({
                            "status": "resolved",
                            "resolved_at": datetime.utcnow()
                        })
            
            db.commit()
            
        except Exception as e:
            print(f"Alert checking failed: {e}")
            db.rollback()
    
    async def run_collection_loop(self):
        """Main collection loop - runs every minute"""
        self.running = True
        print("Starting monitoring collection loop")
        
        while self.running:
            start = time.time()
            
            try:
                results = await self.collect_all()
                self.store_metrics(results)
                
                # Log summary
                success_count = sum(1 for r in results.values() if r.success)
                print(f"Collection cycle complete: {success_count}/4 collectors successful")
                
            except Exception as e:
                print(f"Collection loop error: {e}")
            
            # Sleep until next interval
            elapsed = time.time() - start
            sleep_time = max(0, self.interval - elapsed)
            await asyncio.sleep(sleep_time)
        
        print("Monitoring collection loop stopped")
    
    def stop(self):
        self.running = False


# Singleton instance
monitoring_service = MonitoringService()
