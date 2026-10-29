"""Stable Chrome-extension credential for the embedded sidecar.

The ``/captcha_ws`` endpoint authenticates with the server API key, and in
embedded mode that key is overridden to the **per-launch control token**
(``core.embedded.install_runtime_api_key``).  That is correct for host-to-sidecar
calls, but it makes the Chrome extension unusable: the extension stores its
credentials once in ``chrome.storage.local`` and reconnects on its own, so a
credential that rotates on every launch can never be kept in sync.

This module provisions a *stable*, extension-only credential inside the writable
data dir.  It is:

* embedded-mode only (standalone keeps the historical single API key),
* loopback-only in effect, because the embedded listener never binds elsewhere,
* never logged and never returned by the public API,
* separate from the launch token, the admin password and the DB-stored API key,
  so rotating any of those cannot lock the extension out and leaking the
  extension key cannot authenticate as the host.
"""

from __future__ import annotations

import hmac
import json
import os
import secrets
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, Optional

from . import paths
from .logger import debug_logger

_EXTENSION_KEY_SCHEMA_VERSION = 1
_GENERATED_KEY_BYTES = 24  # ~32 url-safe chars


def _write_private_json(path: Path, payload: Dict[str, Any]) -> None:
    """Atomically write ``payload`` as JSON, restricting permissions where possible."""
    paths.ensure_writable_dir(path.parent)
    temp_path = path.with_suffix(path.suffix + ".part")
    with open(temp_path, "w", encoding="utf-8", newline="\n") as handle:
        json.dump(payload, handle, ensure_ascii=False, indent=2)
        handle.write("\n")
    os.replace(temp_path, path)
    try:
        os.chmod(path, 0o600)
    except Exception:
        # Windows / restricted filesystems: best effort only.
        pass


def _read_private_json(path: Path) -> Optional[Dict[str, Any]]:
    try:
        with open(path, "r", encoding="utf-8") as handle:
            data = json.load(handle)
        return data if isinstance(data, dict) else None
    except FileNotFoundError:
        return None
    except Exception as exc:  # pragma: no cover - defensive
        debug_logger.log_warning(
            f"[Embedded] 读取扩展密钥文件失败 ({path}): {type(exc).__name__}"
        )
        return None


def read_extension_key() -> Optional[str]:
    """Read the existing extension key, if one has been provisioned."""
    key_path = paths.extension_key_file()
    if key_path is None:
        return None
    payload = _read_private_json(key_path)
    if not payload:
        return None
    key = str(payload.get("key") or "").strip()
    return key or None


def ensure_extension_key() -> Optional[str]:
    """Provision the extension key once; return it (``None`` in standalone mode).

    Idempotent: an existing file is authoritative and never regenerated, so a
    restart, an upgrade or a re-run never silently breaks a configured extension.
    """
    key_path = paths.extension_key_file()
    if key_path is None:
        return None

    existing = read_extension_key()
    if existing:
        return existing

    generated = secrets.token_urlsafe(_GENERATED_KEY_BYTES)
    payload = {
        "schema": _EXTENSION_KEY_SCHEMA_VERSION,
        "key": generated,
        "generated_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
    }
    _write_private_json(key_path, payload)
    # Never log the key itself; only note that provisioning happened.
    debug_logger.log_info("[Embedded] 已生成稳定的浏览器扩展密钥（仅本机回环可用）")
    return generated


def _read_admin_api_key_from_db() -> Optional[str]:
    """Fallback: read api_key directly from SQLite admin_config table if available."""
    try:
        import sqlite3

        base_dir = paths.data_dir()
        if not base_dir:
            return None
        candidate_paths = [
            base_dir / "data" / "flow.db",
            base_dir / "flow.db",
        ]
        for db_path in candidate_paths:
            if db_path.is_file():
                with sqlite3.connect(str(db_path)) as conn:
                    cursor = conn.cursor()
                    cursor.execute(
                        "SELECT api_key FROM admin_config WHERE id = 1 LIMIT 1"
                    )
                    row = cursor.fetchone()
                    if row and row[0]:
                        return str(row[0]).strip()
    except Exception:
        pass
    return None


def authorize_extension_credential(candidate: Optional[str]) -> bool:
    """Authorize a ``/captcha_ws`` credential presented by an extension worker.

    Embedded mode accepts:
    - the per-launch control token (host IPC)
    - the stable extension key (from extension-key.json)
    - the user-configured API key (from config._config or admin_config database)
    Standalone mode accepts the configured API key only.
    """
    presented = str(candidate or "").strip()
    if not presented:
        return False

    from . import embedded
    from .config import config

    if not embedded.is_enabled():
        configured = str(getattr(config, "api_key", "") or "").strip()
        if not configured:
            return False
        return hmac.compare_digest(configured, presented)

    # 1. Per-launch host control token
    if embedded.authorize_launch_token(presented):
        return True

    # 2. Stable extension key (extension-key.json)
    key = read_extension_key()
    if key and hmac.compare_digest(key, presented):
        return True

    # 3. User-configured admin API key from memory config (e.g. 1234567)
    raw_cfg = getattr(config, "_config", None)
    if isinstance(raw_cfg, dict):
        configured_api_key = str(
            raw_cfg.get("global", {}).get("api_key", "") or ""
        ).strip()
        if configured_api_key and hmac.compare_digest(configured_api_key, presented):
            return True

    # 4. Fallback directly from SQLite admin_config
    db_api_key = _read_admin_api_key_from_db()
    if db_api_key and hmac.compare_digest(db_api_key, presented):
        return True

    return False


__all__ = [
    "read_extension_key",
    "ensure_extension_key",
    "authorize_extension_credential",
]
