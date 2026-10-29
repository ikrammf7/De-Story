"""Embedded sidecar entry point for Flow2API.

This module is the process entry point used by the host (Rust/Tauri) launcher.
It is intentionally run *isolated* so the bundled code tree cannot be shadowed by
stray modules on ``sys.path``:

    python -I -u <absolute path to embedded_main.py> --data-dir <dir>

Because ``-I`` (Python 3.11+) also implies ``-P``, this script manually inserts
its own repository/source root into ``sys.path`` so ``src`` and its dependencies
(bundled in the same tree / interpreter) can be imported.

Contract
--------
argv
    ``--data-dir <dir>``  required; every writable location is redirected here.
    ``--host <host>``     loopback host to bind (default 127.0.0.1).
    ``--port <n>``        fixed port to bind, or 0 (default) for an ephemeral
                          port. Only needed when a client must know the address
                          in advance (e.g. a Chrome extension worker); the bound
                          port is reported on stdout in both cases.

stdin (line-delimited JSON)
    1. first line: the launch handshake, e.g.
       ``{"token": "<random per-launch control token>", "instance": "<id>"}``
    2. later lines: ``{"command": "shutdown"}`` ... or EOF (host died) -> stop.

stdout
    A single structured readiness line once Uvicorn is accepting connections::

        BANG_FLOW_READY {"port": <int>, "protocol": 1, "instance": "<id>"}

    The launch token is deliberately **never** written to stdout; the host
    authenticates ``GET /_bang/ready`` with it via ``Authorization: Bearer``.

The listener is created here (loopback, ephemeral port) and handed to Uvicorn as
a pre-bound socket so the port is known before startup completes.
"""

from __future__ import annotations

import argparse
import asyncio
import json
import os
import socket
import sys
import threading
import time
from pathlib import Path
from typing import Any, Dict, Optional, Tuple

_REPO_ROOT = Path(__file__).resolve().parent


def _bootstrap_source_root() -> None:
    """Insert this bundle's root so ``src`` is importable under ``-I``."""
    root = str(_REPO_ROOT)
    if root and root not in sys.path:
        sys.path.insert(0, root)


def _parse_args(argv: Optional[list] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(prog="embedded_main.py", add_help=True)
    parser.add_argument(
        "--data-dir",
        required=True,
        help="Writable data directory (config/data/tmp/logs live here).",
    )
    parser.add_argument(
        "--host",
        default="127.0.0.1",
        help="Loopback host to bind (default 127.0.0.1).",
    )
    parser.add_argument(
        "--port",
        type=int,
        default=0,
        help=(
            "Fixed loopback port to bind, or 0 for an OS-assigned ephemeral port "
            "(default 0). A fixed port is only needed when an external client "
            "must know the address in advance, e.g. a Chrome extension worker; "
            "the port is still reported on stdout either way."
        ),
    )
    return parser.parse_args(argv)


def _fail(message: str, *, code: int = 1) -> "None":
    # Diagnostics go to stderr; stdout is reserved for the readiness line.
    print(f"[embedded_main] {message}", file=sys.stderr, flush=True)
    raise SystemExit(code)


def _read_handshake() -> Tuple[str, str]:
    """Read the first JSON line from stdin: ``{"token", "instance"}``."""
    raw = sys.stdin.readline()
    if not raw:
        _fail("no launch handshake received on stdin")
    try:
        payload = json.loads(raw)
    except Exception as exc:  # noqa: BLE001
        _fail(f"invalid launch handshake JSON: {type(exc).__name__}")
    if not isinstance(payload, dict):
        _fail("launch handshake must be a JSON object")
    token = str(payload.get("token") or "").strip()
    instance = str(payload.get("instance") or "")
    if not token:
        _fail("launch handshake is missing a non-empty 'token'")
    return token, instance


_FIXED_PORT_BIND_ATTEMPTS = 40
_FIXED_PORT_BIND_INTERVAL = 0.25  # seconds; ~10 s total


def _bind_once(host: str, port: int) -> socket.socket:
    family = socket.AF_INET6 if ":" in host else socket.AF_INET
    listener = socket.socket(family, socket.SOCK_STREAM)
    if port:
        # Do not set SO_REUSEADDR for a fixed port on Windows: it would let a
        # second instance "succeed" by stealing an address already in use.
        if os.name != "nt":
            listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    else:
        listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    try:
        listener.bind((host, int(port or 0)))
    except OSError:
        listener.close()
        raise
    listener.listen(128)
    listener.set_inheritable(True)
    return listener


def _create_listener(host: str, port: int = 0) -> socket.socket:
    """Bind the loopback listener.

    ``port == 0`` asks the OS for an ephemeral port, which is safe by default.

    A caller-supplied port is bound as-is. If it cannot be bound the launcher
    fails with a clear message instead of silently falling back to a port the
    caller would never learn about. The bind is retried briefly first, because a
    restart of *our own* previous child (or Windows' short-lived reservation of a
    just-released address) can hold the port for a moment without this being a
    real conflict.
    """
    if not port:
        return _bind_once(host, 0)

    last_error: Optional[OSError] = None
    for attempt in range(_FIXED_PORT_BIND_ATTEMPTS):
        try:
            return _bind_once(host, port)
        except OSError as exc:
            last_error = exc
            if attempt + 1 < _FIXED_PORT_BIND_ATTEMPTS:
                time.sleep(_FIXED_PORT_BIND_INTERVAL)

    _fail(
        f"cannot bind fixed port {port} on {host}: "
        f"{getattr(last_error, 'strerror', None) or last_error}. "
        "Another process may already use it; stop that process or pick "
        "a different port."
    )
    raise AssertionError("unreachable")  # _fail always raises


def _start_control_reader(server_holder: Dict[str, Any]) -> threading.Thread:
    """Background stdin reader: shutdown command or EOF -> graceful stop."""

    def _loop() -> None:
        while True:
            try:
                raw = sys.stdin.readline()
            except Exception:
                raw = ""
            if not raw:
                # EOF: host closed the pipe (crashed or exited) -> stop.
                break
            stripped = raw.strip()
            if not stripped:
                continue
            try:
                command = json.loads(stripped)
            except Exception:
                continue
            if isinstance(command, dict) and str(
                command.get("command") or ""
            ).strip().lower() == "shutdown":
                break
        server = server_holder.get("server")
        if server is not None:
            server.should_exit = True

    thread = threading.Thread(target=_loop, name="embedded-control", daemon=True)
    thread.start()
    return thread


def _emit_ready(*, port: int, protocol: int, instance: str) -> None:
    payload = {"port": int(port), "protocol": int(protocol), "instance": instance}
    from src.core.embedded import READY_STDOUT_PREFIX

    sys.stdout.write(f"{READY_STDOUT_PREFIX} {json.dumps(payload)}\n")
    sys.stdout.flush()


async def _serve(socket_handle: socket.socket, token: str, instance: str) -> None:
    import uvicorn

    from src.core import embedded

    embedded.configure(launch_token=token, instance=instance)
    port = int(socket_handle.getsockname()[1])
    embedded.set_bound_port(port)

    config = uvicorn.Config(
        "src.main:app",
        log_level="info",
        access_log=False,
        lifespan="on",
    )
    server = uvicorn.Server(config)
    holder: Dict[str, Any] = {"server": server}
    _start_control_reader(holder)

    serve_task = asyncio.create_task(server.serve(sockets=[socket_handle]))
    try:
        # Wait until the ASGI app has started (lifespan startup complete).
        while not server.started:
            if serve_task.done():
                serve_task.result()  # surface startup exception
                _fail("uvicorn exited before becoming ready")
            await asyncio.sleep(0.05)
        embedded.mark_ready()
        _emit_ready(port=port, protocol=embedded.protocol(), instance=instance)
        await serve_task
    finally:
        if not serve_task.done():
            server.should_exit = True
            try:
                await serve_task
            except Exception:
                pass


def main(argv: Optional[list] = None) -> int:
    args = _parse_args(argv)
    data_dir = Path(args.data_dir).expanduser().resolve()
    try:
        data_dir.mkdir(parents=True, exist_ok=True)
    except Exception as exc:  # noqa: BLE001
        _fail(f"cannot create data dir {data_dir}: {type(exc).__name__}")

    # Redirect every writable location before importing the app.
    os.environ["FLOW2API_DATA_DIR"] = str(data_dir)
    os.environ.setdefault("PYTHONIOENCODING", "utf-8")

    _bootstrap_source_root()

    token, instance = _read_handshake()

    listener = _create_listener(args.host, args.port)
    try:
        asyncio.run(_serve(listener, token, instance))
    except KeyboardInterrupt:
        return 0
    finally:
        try:
            listener.close()
        except Exception:
            pass
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
