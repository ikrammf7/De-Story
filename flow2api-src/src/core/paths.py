"""Path abstractions for bundled (read-only) code and writable runtime data.

Flow2API normally runs from a source checkout where ``config/``, ``data/``,
``tmp/`` and ``static/`` live next to ``main.py``.  When the backend is launched
as an *embedded* sidecar (see ``embedded_main.py`` and ``.core.embedded``) the
code is bundled read-only and every writable location is redirected into a
single data directory supplied by the host process:

    FLOW2API_DATA_DIR/
        config/setting.toml        (placed by the host; read here, never written)
        data/flow.db               (placed/migrated by the host)
        data/browser_profile/      (default browser profile root)
        tmp/                       (file cache + reCAPTCHA scratch assets)
        logs.txt

In standalone mode (no ``FLOW2API_DATA_DIR``) the historical behaviour is
preserved: everything resolves relative to the repository root derived from the
``src/`` package location.  This module is the single source of truth for those
two layouts so the rest of the codebase never has to branch on the mode.
"""

from __future__ import annotations

import os
from pathlib import Path
from typing import Optional

# Environment variable set by the host launcher (Rust) to point at the writable
# data directory.  Absent/empty means "run standalone".
DATA_DIR_ENV_VAR = "FLOW2API_DATA_DIR"

# ``src/`` package directory and the repository root (its parent).
_SRC_ROOT = Path(__file__).resolve().parent.parent
_REPO_ROOT = _SRC_ROOT.parent


def _normalized_env_path(name: str) -> Optional[Path]:
    raw = str(os.environ.get(name, "") or "").strip()
    if not raw:
        return None
    try:
        return Path(raw).expanduser().resolve()
    except Exception:
        return Path(raw)


# Resolved once at import time.  Empty/None in standalone mode.
_DATA_DIR: Optional[Path] = _normalized_env_path(DATA_DIR_ENV_VAR)


def is_embedded() -> bool:
    """Return True when running as an embedded sidecar with a data directory."""
    return _DATA_DIR is not None


def data_dir() -> Optional[Path]:
    """Writable data directory in embedded mode, else None."""
    return _DATA_DIR


def src_root() -> Path:
    """Location of the ``src`` package (read-only bundled code)."""
    return _SRC_ROOT


def repo_root() -> Path:
    """Read-only repository/bundle root in standalone mode, else ``src`` parent."""
    return _REPO_ROOT


# Backwards-compatible alias used by existing call sites.
RUNTIME_ROOT = _SRC_ROOT


def bundled_root() -> Path:
    """Root of the bundled read-only code (always the repository root)."""
    return _REPO_ROOT


# ---------------------------------------------------------------------------
# Read-only bundled assets
# ---------------------------------------------------------------------------
def config_dir() -> Path:
    """Directory holding ``setting.toml`` / ``setting_example.toml``."""
    if _DATA_DIR is not None:
        return _DATA_DIR / "config"
    return _REPO_ROOT / "config"


def config_file() -> Path:
    """Primary config file (``setting.toml``)."""
    return config_dir() / "setting.toml"


def config_example_file() -> Path:
    """Fallback/bundled example config file."""
    return config_dir() / "setting_example.toml"


def bundled_config_example_file() -> Path:
    """Bundled example config shipped read-only with the code."""
    return _REPO_ROOT / "config" / "setting_example.toml"


def static_dir() -> Path:
    """Frontend static assets directory (read-only in embedded mode)."""
    if _DATA_DIR is not None:
        return _REPO_ROOT / "static"
    return _REPO_ROOT / "static"


def static_path(file_name: str) -> Path:
    return static_dir() / file_name


# ---------------------------------------------------------------------------
# Writable locations
# ---------------------------------------------------------------------------
def writable_data_root() -> Path:
    """Root writable directory: the data dir in embedded mode, else repo/``data``."""
    if _DATA_DIR is not None:
        return _DATA_DIR
    return _REPO_ROOT / "data"


def tmp_dir() -> Path:
    """Writable scratch/cache directory mounted at ``/tmp``."""
    if _DATA_DIR is not None:
        return _DATA_DIR / "tmp"
    return _REPO_ROOT / "tmp"


def data_store_dir() -> Path:
    """Directory holding the SQLite database and default browser profile."""
    if _DATA_DIR is not None:
        return _DATA_DIR / "data"
    return _REPO_ROOT / "data"


def db_path() -> Path:
    """SQLite database path (host-owned in embedded mode)."""
    if _DATA_DIR is not None:
        return _DATA_DIR / "data" / "flow.db"
    return _REPO_ROOT / "data" / "flow.db"


def browser_profile_dir() -> Path:
    """Default browser user-data-dir root."""
    if _DATA_DIR is not None:
        return _DATA_DIR / "data" / "browser_profile"
    return _REPO_ROOT / "data" / "browser_profile"


def logs_file() -> Path:
    """Debug log file path."""
    if _DATA_DIR is not None:
        return _DATA_DIR / "logs.txt"
    return Path("logs.txt")


def admin_credentials_file() -> Optional[Path]:
    """Private admin credentials file written for fresh/embedded databases."""
    if _DATA_DIR is not None:
        return _DATA_DIR / "admin-credentials.json"
    return None


def extension_key_file() -> Optional[Path]:
    """Private, stable extension-worker key file (embedded mode only).

    Deliberately distinct from the per-launch control token: the Chrome
    extension configures its credentials once and cannot follow a token that
    changes on every launch.
    """
    if _DATA_DIR is not None:
        return _DATA_DIR / "extension-key.json"
    return None


def ensure_writable_dir(path: Path) -> Path:
    """Create a writable directory (and parents) tolerating races."""
    try:
        Path(path).mkdir(parents=True, exist_ok=True)
    except FileExistsError:
        pass
    return Path(path)


__all__ = [
    "DATA_DIR_ENV_VAR",
    "is_embedded",
    "data_dir",
    "src_root",
    "repo_root",
    "bundled_root",
    "config_dir",
    "config_file",
    "config_example_file",
    "bundled_config_example_file",
    "static_dir",
    "static_path",
    "writable_data_root",
    "tmp_dir",
    "data_store_dir",
    "db_path",
    "browser_profile_dir",
    "logs_file",
    "admin_credentials_file",
    "extension_key_file",
    "ensure_writable_dir",
    "RUNTIME_ROOT",
]
