"""Embedded sidecar runtime state and control.

This module is shared between ``embedded_main.py`` (the process entry point used
by the host launcher) and the FastAPI app.  It holds the per-launch control
token, the host-supplied instance identifier, the readiness protocol version and
a runtime-metadata readiness tracker that is deliberately separate from the
browser runtime configuration status (``core.browser_runtime_status``).

Nothing here writes to stdout; the launcher owns the ``BANG_FLOW_READY`` line.
"""

from __future__ import annotations

import hmac
import threading
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any, Dict, List, Optional

from . import paths
from .logger import debug_logger

# Wire protocol version understood by host and sidecar.
PROTOCOL_VERSION = 1

# Prefix the launcher writes to stdout once Uvicorn is accepting connections.
READY_STDOUT_PREFIX = "BANG_FLOW_READY"

# Header used by the host to authenticate the readiness probe.
READY_AUTH_SCHEME = "Bearer"


def _now_iso() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


@dataclass
class EmbeddedState:
    """Mutable per-launch embedded state."""

    enabled: bool = False
    launch_token: str = ""
    instance: str = ""
    protocol: int = PROTOCOL_VERSION
    data_dir: str = ""
    started_at: str = ""
    ready: bool = False
    ready_metadata: Dict[str, Any] = field(default_factory=dict)
    bound_port: int = 0


_LOCK = threading.Lock()
_STATE = EmbeddedState()


# ---------------------------------------------------------------------------
# Runtime *metadata* readiness (distinct from browser config readiness)
# ---------------------------------------------------------------------------
_DEFAULT_RUNTIME_READINESS: Dict[str, Any] = {
    "category": "runtime",
    "state": "idle",
    "ready": False,
    "message": "",
    "updated_at": None,
}
_RUNTIME_READINESS: Dict[str, Any] = dict(_DEFAULT_RUNTIME_READINESS)
_READINESS_LOCK = threading.Lock()


def set_runtime_readiness(state: str, message: str = "", *, ready: bool = False) -> Dict[str, Any]:
    with _READINESS_LOCK:
        _RUNTIME_READINESS.update(
            {
                "state": state,
                "ready": bool(ready),
                "message": message,
                "updated_at": _now_iso(),
            }
        )
        return dict(_RUNTIME_READINESS)


def get_runtime_readiness() -> Dict[str, Any]:
    with _READINESS_LOCK:
        return dict(_RUNTIME_READINESS)


# ---------------------------------------------------------------------------
# Launch state
# ---------------------------------------------------------------------------
def configure(*, launch_token: str, instance: str, protocol: int = PROTOCOL_VERSION) -> None:
    """Record the per-launch control token / instance from the host handshake."""
    with _LOCK:
        _STATE.enabled = True
        _STATE.launch_token = str(launch_token or "")
        _STATE.instance = str(instance or "")
        _STATE.protocol = int(protocol or PROTOCOL_VERSION)
        _STATE.data_dir = str(paths.data_dir() or "")
        _STATE.started_at = _now_iso()


def is_enabled() -> bool:
    with _LOCK:
        return _STATE.enabled


def launch_token() -> str:
    with _LOCK:
        return _STATE.launch_token


def instance() -> str:
    with _LOCK:
        return _STATE.instance


def protocol() -> int:
    with _LOCK:
        return _STATE.protocol


def set_bound_port(port: int) -> None:
    with _LOCK:
        _STATE.bound_port = int(port)


def bound_port() -> int:
    with _LOCK:
        return _STATE.bound_port


def base_url() -> str:
    return f"http://127.0.0.1:{bound_port()}"


def mark_ready(metadata: Optional[Dict[str, Any]] = None) -> None:
    with _LOCK:
        _STATE.ready = True
        if metadata:
            _STATE.ready_metadata = dict(metadata)
    set_runtime_readiness("ready", "API runtime is ready", ready=True)


def is_ready() -> bool:
    with _LOCK:
        return _STATE.ready


def authorize_launch_token(candidate: Optional[str]) -> bool:
    """Constant-time comparison of a presented token against the launch token."""
    expected = launch_token()
    presented = str(candidate or "")
    if not expected or not presented:
        return False
    return hmac.compare_digest(expected, presented)


def ready_payload() -> Dict[str, Any]:
    """Body returned by ``GET /_bang/ready`` (never contains the token)."""
    return {
        "protocol": protocol(),
        "instance": instance(),
        "ready": is_ready(),
        "runtime": get_runtime_readiness(),
    }


def install_runtime_api_key() -> bool:
    """Force ``config.api_key`` to the per-launch token in embedded mode.

    The getter is overridden (rather than assigning the property) so that a
    later ``db.reload_config_to_memory()`` cannot overwrite it, while the API key
    stored in the database settings is left untouched.
    """
    from .config import config, Config

    token = launch_token()
    if not is_enabled() or not token:
        return False

    if getattr(Config, "_embedded_api_key_installed", False):
        return True

    def _embedded_api_key_getter(self) -> str:  # noqa: D401 - property getter
        return launch_token()

    Config.api_key = property(_embedded_api_key_getter, Config.api_key.fset)
    Config._embedded_api_key_installed = True
    debug_logger.log_info(
        "[Embedded] 已将运行时 API Key 绑定到本次启动的控制令牌（数据库设置保持不变）"
    )
    # `config` is a module-level singleton; ensure any cached value reflects it.
    try:
        _ = config.api_key
    except Exception:
        pass
    return True


def strict_cors_origins() -> List[str]:
    """Origins allowed in embedded mode (loopback only)."""
    return [
        "http://127.0.0.1",
        "http://localhost",
        "http://[::1]",
        "https://127.0.0.1",
        "https://localhost",
        "https://[::1]",
    ]


__all__ = [
    "PROTOCOL_VERSION",
    "READY_STDOUT_PREFIX",
    "READY_AUTH_SCHEME",
    "configure",
    "is_enabled",
    "launch_token",
    "instance",
    "protocol",
    "set_bound_port",
    "bound_port",
    "base_url",
    "mark_ready",
    "is_ready",
    "authorize_launch_token",
    "ready_payload",
    "install_runtime_api_key",
    "strict_cors_origins",
    "set_runtime_readiness",
    "get_runtime_readiness",
]
