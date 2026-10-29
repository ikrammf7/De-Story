"""Embedded-mode admin credential provisioning.

When Flow2API runs as an embedded sidecar the host (Rust) owns the database and
config migration, but it must never leave the historical insecure ``admin/admin``
default active.  This module:

* detects fresh databases and databases still carrying the insecure default,
* generates a strong random admin password,
* persists it to a *private* file inside ``FLOW2API_DATA_DIR`` so the host/UI can
  surface it later without the credentials ever appearing in the logs,
* reconciles the in-memory + database admin config to that value.

It is a no-op in standalone mode (no ``FLOW2API_DATA_DIR``).
"""

from __future__ import annotations

import json
import os
import secrets
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, Optional

from . import paths
from .logger import debug_logger

# The historical insecure default baked into setting_example.toml / first-run DB.
DEFAULT_ADMIN_USERNAME = "admin"
DEFAULT_ADMIN_PASSWORD = "admin"

_CREDENTIALS_SCHEMA_VERSION = 1
_GENERATED_PASSWORD_BYTES = 18  # ~24 url-safe chars


def _is_default_admin(username: Optional[str], password: Optional[str]) -> bool:
    return str(username or "").strip() == DEFAULT_ADMIN_USERNAME and str(
        password or ""
    ) == DEFAULT_ADMIN_PASSWORD


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
            f"[Embedded] 读取管理员凭证文件失败 ({path}): {type(exc).__name__}"
        )
        return None


async def ensure_admin_credentials(db) -> Optional[Dict[str, Any]]:
    """Ensure a non-default admin credential exists for embedded mode.

    Returns the effective credential dict (``{"username", "password"}``) when a
    private file is in use, else ``None`` (standalone mode).
    """
    credentials_path = paths.admin_credentials_file()
    if credentials_path is None:
        # Standalone: keep the historical behaviour untouched.
        return None

    admin_config = await db.get_admin_config()
    if admin_config is None:
        return None

    existing_file = _read_private_json(credentials_path)
    if existing_file:
        username = str(existing_file.get("username") or DEFAULT_ADMIN_USERNAME)
        password = str(existing_file.get("password") or "")
        if password:
            # File is authoritative: keep the DB/in-memory config in sync so a
            # restart can never fall back to a stale/default password.
            if (
                admin_config.username != username
                or admin_config.password != password
            ):
                await db.update_admin_config(username=username, password=password)
                await db.reload_config_to_memory()
            return {"username": username, "password": password}

    if not _is_default_admin(admin_config.username, admin_config.password):
        # A user-chosen password already exists and no private file is present.
        # Leave it alone; the private file is only provisioned for fresh/default
        # credentials.
        return None

    username = (
        str(admin_config.username or "").strip() or DEFAULT_ADMIN_USERNAME
    )
    generated_password = secrets.token_urlsafe(_GENERATED_PASSWORD_BYTES)
    payload = {
        "schema": _CREDENTIALS_SCHEMA_VERSION,
        "username": username,
        "password": generated_password,
        "generated_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
    }
    _write_private_json(credentials_path, payload)
    await db.update_admin_config(username=username, password=generated_password)
    await db.reload_config_to_memory()
    # Never log the credential itself; only note that provisioning happened.
    debug_logger.log_info(
        "[Embedded] 已为默认管理员生成随机密码并写入私有凭证文件"
    )
    return {"username": username, "password": generated_password}


def read_admin_credentials() -> Optional[Dict[str, str]]:
    """Read the private admin credential file (host/UI convenience)."""
    credentials_path = paths.admin_credentials_file()
    if credentials_path is None:
        return None
    payload = _read_private_json(credentials_path)
    if not payload:
        return None
    username = str(payload.get("username") or "")
    password = str(payload.get("password") or "")
    if not username or not password:
        return None
    return {"username": username, "password": password}


__all__ = [
    "DEFAULT_ADMIN_USERNAME",
    "DEFAULT_ADMIN_PASSWORD",
    "ensure_admin_credentials",
    "read_admin_credentials",
]
