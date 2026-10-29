"""Browser runtime preparation status helpers."""

from __future__ import annotations

from copy import deepcopy
from datetime import datetime
from threading import Lock
from typing import Any, Dict


_STATUS_LOCK = Lock()
_DEFAULT_STATUS = {
    "state": "idle",
    "active": False,
    "message": "",
    "error": "",
    "updated_at": None,
    "last_completed_at": None,
    "sequence": 0,
}
_RUNTIME_STATUS: Dict[str, Dict[str, Any]] = {
    "browser": dict(_DEFAULT_STATUS),
    "personal": dict(_DEFAULT_STATUS),
}


def _now_iso() -> str:
    return datetime.now().isoformat(timespec="seconds")


def _normalize_runtime_kind(runtime_kind: str) -> str:
    kind = (runtime_kind or "").strip().lower()
    if kind not in _RUNTIME_STATUS:
        raise ValueError(f"Unsupported runtime kind: {runtime_kind}")
    return kind


def _update_status(runtime_kind: str, **fields: Any) -> Dict[str, Any]:
    kind = _normalize_runtime_kind(runtime_kind)
    with _STATUS_LOCK:
        status = _RUNTIME_STATUS[kind]
        status.update(fields)
        status["updated_at"] = _now_iso()
        status["sequence"] = int(status.get("sequence", 0) or 0) + 1
        if status.get("state") == "ready":
            status["last_completed_at"] = status["updated_at"]
        return deepcopy(status)


def start_runtime_prepare(runtime_kind: str, message: str) -> Dict[str, Any]:
    return _update_status(
        runtime_kind,
        state="running",
        active=True,
        message=message,
        error="",
    )


def progress_runtime_prepare(runtime_kind: str, message: str) -> Dict[str, Any]:
    return _update_status(
        runtime_kind,
        state="running",
        active=True,
        message=message,
    )


def finish_runtime_prepare(runtime_kind: str, message: str) -> Dict[str, Any]:
    return _update_status(
        runtime_kind,
        state="ready",
        active=False,
        message=message,
        error="",
    )


def fail_runtime_prepare(runtime_kind: str, message: str) -> Dict[str, Any]:
    return _update_status(
        runtime_kind,
        state="error",
        active=False,
        message=message,
        error=message,
    )


def reset_runtime_prepare(runtime_kind: str, message: str = "") -> Dict[str, Any]:
    return _update_status(
        runtime_kind,
        state="idle",
        active=False,
        message=message,
        error="",
    )


def get_runtime_status(runtime_kind: str) -> Dict[str, Any]:
    kind = _normalize_runtime_kind(runtime_kind)
    with _STATUS_LOCK:
        return deepcopy(_RUNTIME_STATUS[kind])
