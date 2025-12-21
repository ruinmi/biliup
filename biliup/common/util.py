import asyncio
import json
import logging
import re
from datetime import datetime, timezone

import httpx

try:
    import ssl
    import truststore  # type: ignore
except ImportError:
    ssl = None
    truststore = None
    _ssl_context = True
else:
    _ssl_context = truststore.SSLContext(ssl.PROTOCOL_TLS_CLIENT)

DEFAULT_TIMEOUT = httpx.Timeout(
    connect=15.0,
    read=60.0,
    write=60.0,
    pool=15.0,
)
DEFAULT_MAX_RETRIES = 2
DEFAULT_CONNECTION_LIMITS = httpx.Limits(max_connections=100, max_keepalive_connections=100)

client = httpx.AsyncClient(
    http2=True,
    follow_redirects=True,
    timeout=DEFAULT_TIMEOUT,
    limits=DEFAULT_CONNECTION_LIMITS,
    verify=_ssl_context,
)
loop = asyncio.get_running_loop()
logger = logging.getLogger('biliup')


def _parse_time_value(value):
    """
    将单个时间值解析为 datetime.time，支持 ISO 时间、仅时分、时分秒格式
    """
    if not isinstance(value, str):
        return None
    text = value.strip().replace('Z', '+00:00')

    for parser in (
        lambda v: datetime.fromisoformat(v).time(),
        lambda v: datetime.strptime(v, "%H:%M").time(),
        lambda v: datetime.strptime(v, "%H:%M:%S").time(),
    ):
        try:
            return parser(text).replace(second=0, microsecond=0)
        except Exception:
            continue
    return None


def parse_time_range(time_range_raw):
    """
    解析配置中的时间范围为 (start, end)，支持 JSON 数组字符串、列表/元组，也支持 'HH:MM-HH:MM' 格式
    """
    parsed = None

    if isinstance(time_range_raw, (list, tuple)):
        parsed = time_range_raw
    elif isinstance(time_range_raw, str):
        try:
            parsed = json.loads(time_range_raw)
        except Exception:
            parsed = None

    if isinstance(parsed, (list, tuple)) and len(parsed) == 2:
        start = _parse_time_value(parsed[0])
        end = _parse_time_value(parsed[1])
        if start and end:
            return start, end

    if isinstance(time_range_raw, str):
        match = re.match(r'^\s*([\d:]+)\s*-\s*([\d:]+)\s*$', time_range_raw)
        if match:
            start = _parse_time_value(match.group(1))
            end = _parse_time_value(match.group(2))
            if start and end:
                return start, end

    return None


def check_timerange(name):
    from biliup.config import config

    try:
        time_range_raw = config['streamers'].get(name, {}).get('time_range')
        if not time_range_raw:
            return True

        parsed = parse_time_range(time_range_raw)
        if not parsed:
            return True

        start, end = parsed
    except Exception as e:
        logger.error(f'parsing time range {e}')
        return True

    now = datetime.now(timezone.utc).time().replace(second=0, microsecond=0)

    # Normal interval (e.g. 16:00 -> 20:00)
    if start <= end:
        return start <= now <= end

    # Cross-midnight (e.g. 23:00 -> 04:00)
    return now >= start or now <= end
