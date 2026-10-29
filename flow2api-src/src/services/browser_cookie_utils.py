"""Helpers for normalizing and parsing browser cookies."""

from __future__ import annotations

import hashlib
import json
import re
import time
from typing import Any, Dict, List, Optional
from urllib.parse import unquote, urlparse

DEFAULT_COOKIE_URL = "https://flow.google.com/"
DEFAULT_GOOGLE_COOKIE_TARGET_URLS = (
    "https://flow.google.com/",
    "https://www.google.com/",
)
_COOKIE_ATTRIBUTE_KEYS = {
    "path",
    "domain",
    "expires",
    "max-age",
    "secure",
    "httponly",
    "samesite",
    "priority",
    "partitioned",
}
_SESSION_TOKEN_COOKIE_NAMES = (
    "__Secure-next-auth.session-token",
    "next-auth.session-token",
)
FLOW_SESSION_COOKIE_NAMES = frozenset({"OSID", "__Secure-OSID"})
GOOGLE_ACCOUNT_COOKIE_NAMES = frozenset(
    {
        "SID",
        "HSID",
        "SSID",
        "APISID",
        "SAPISID",
        "__Secure-1PSID",
        "__Secure-3PSID",
    }
)
_SAFE_COOKIE_NAME = re.compile(r"[!#$%&'*+\-.^_`|~0-9A-Za-z]+")


def normalize_cookie_header_text(raw_cookie: Optional[str]) -> str:
    value = str(raw_cookie or "").strip()
    if not value:
        return ""
    if value.lower().startswith("cookie:"):
        value = value.split(":", 1)[1].strip()
    return value


def normalize_cookie_storage_text(raw_cookie: Any) -> str:
    if raw_cookie is None:
        return ""
    if isinstance(raw_cookie, (dict, list)):
        try:
            return json.dumps(raw_cookie, ensure_ascii=False, separators=(",", ":"))
        except Exception:
            return ""
    value = str(raw_cookie).strip()
    if not value:
        return ""
    if value[:1] in {"[", "{"}:
        try:
            payload = json.loads(value)
        except Exception:
            payload = None
        if isinstance(payload, (dict, list)):
            try:
                return json.dumps(payload, ensure_ascii=False, separators=(",", ":"))
            except Exception:
                return value
    return normalize_cookie_header_text(value)


def _normalize_same_site(value: Any) -> Optional[str]:
    raw = str(value or "").strip().lower()
    if raw == "strict":
        return "Strict"
    if raw == "none":
        return "None"
    if raw == "lax":
        return "Lax"
    return None


def is_partitioned_cookie(cookie: Any) -> bool:
    """Detect browser partition scope that an ordinary Cookie jar cannot retain."""
    keys = ("partitionKey", "partition_key")
    flags = ("partitionKeyOpaque", "partition_key_opaque", "partitioned")
    if isinstance(cookie, dict):
        return any(cookie.get(key) is not None for key in keys) or any(
            bool(cookie.get(flag)) for flag in flags
        )
    return any(getattr(cookie, key, None) is not None for key in keys) or any(
        bool(getattr(cookie, flag, False)) for flag in flags
    )


def _build_cookie_from_mapping(
    raw_cookie: Dict[str, Any], default_url: str
) -> Optional[Dict[str, Any]]:
    if is_partitioned_cookie(raw_cookie):
        return None
    name = str(raw_cookie.get("name") or "").strip()
    if not name:
        return None
    cookie: Dict[str, Any] = {
        "name": name,
        "value": str(raw_cookie.get("value") or ""),
    }
    domain = str(raw_cookie.get("domain") or "").strip()
    url = str(raw_cookie.get("url") or "").strip()
    path = str(raw_cookie.get("path") or "/").strip() or "/"
    if url:
        cookie["url"] = url
        if raw_cookie.get("path"):
            cookie["path"] = path
    elif domain:
        host_only = raw_cookie.get("hostOnly")
        if host_only is True or (host_only is None and not domain.startswith(".")):
            scheme = "https" if raw_cookie.get("secure", True) else "http"
            cookie["url"] = f"{scheme}://{domain.lstrip('.')}/"
        else:
            cookie["domain"] = domain
        cookie["path"] = path
    else:
        cookie["url"] = default_url
        cookie["path"] = path
    same_site = _normalize_same_site(raw_cookie.get("sameSite"))
    if same_site:
        cookie["sameSite"] = same_site
    expires = raw_cookie.get("expires", raw_cookie.get("expirationDate"))
    if expires not in (None, ""):
        try:
            cookie["expires"] = float(expires)
        except Exception:
            pass
    for key in ("secure", "httpOnly"):
        if key in raw_cookie:
            cookie[key] = bool(raw_cookie.get(key))
    if name.startswith("__Secure-") or name.startswith("__Host-"):
        cookie["secure"] = True
    if name.startswith("__Host-"):
        cookie.pop("domain", None)
        cookie["path"] = "/"
        origin = urlparse(cookie.get("url") or (f"https://{domain.lstrip('.')}/" if domain else default_url))
        cookie["url"] = f"https://{origin.netloc}/" if origin.netloc else default_url
    return cookie


def parse_browser_cookie_payload(
    raw_cookie: Any, default_url: str = DEFAULT_COOKIE_URL
) -> List[Dict[str, Any]]:
    normalized = normalize_cookie_storage_text(raw_cookie)
    if not normalized:
        return []
    if normalized[:1] in {"[", "{"}:
        try:
            payload = json.loads(normalized)
        except Exception:
            payload = None
        if isinstance(payload, dict) and isinstance(payload.get("cookies"), list):
            payload = payload["cookies"]
        elif isinstance(payload, dict):
            payload = [payload]
        if isinstance(payload, list):
            cookies: List[Dict[str, Any]] = []
            for item in payload:
                if not isinstance(item, dict):
                    continue
                normalized_item = _build_cookie_from_mapping(item, default_url)
                if normalized_item:
                    cookies.append(normalized_item)
            return cookies
    cookies = []
    for chunk in normalized.replace("\n", ";").split(";"):
        segment = chunk.strip()
        if not segment or "=" not in segment:
            continue
        name, value = segment.split("=", 1)
        cookie_name = name.strip()
        if not cookie_name or cookie_name.lower() in _COOKIE_ATTRIBUTE_KEYS:
            continue
        cookie: Dict[str, Any] = {
            "name": cookie_name,
            "value": value.strip(),
            "url": default_url,
            "path": "/",
            "secure": default_url.startswith("https://"),
        }
        if cookie_name.startswith("__Secure-") or cookie_name.startswith("__Host-"):
            cookie["secure"] = True
        if cookie_name.startswith("__Host-"):
            cookie["path"] = "/"
        cookies.append(cookie)
    return cookies


def build_browser_cookie_targets(
    raw_cookie: Any,
    default_url: str = DEFAULT_COOKIE_URL,
    fallback_urls: Optional[List[str]] = None,
) -> List[Dict[str, Any]]:
    normalized = normalize_cookie_storage_text(raw_cookie)
    if not normalized:
        return []
    target_urls = (
        tuple(
        dict.fromkeys(
            [
                str(url or "").strip()
                    for url in (
                        fallback_urls or list(DEFAULT_GOOGLE_COOKIE_TARGET_URLS)
                    )
                if str(url or "").strip()
            ]
        )
        )
        or DEFAULT_GOOGLE_COOKIE_TARGET_URLS
    )
    expanded: List[Dict[str, Any]] = []
    seen: set[str] = set()

    def append_cookie(cookie: Dict[str, Any]):
        stable_key = json.dumps(
            cookie, ensure_ascii=True, sort_keys=True, separators=(",", ":")
        )
        if stable_key in seen:
            return
        seen.add(stable_key)
        expanded.append(cookie)

    if normalized[:1] in {"[", "{"}:
        try:
            payload = json.loads(normalized)
        except Exception:
            payload = None
        if isinstance(payload, dict) and isinstance(payload.get("cookies"), list):
            payload = payload["cookies"]
        elif isinstance(payload, dict):
            payload = [payload]
        if isinstance(payload, list):
            for item in payload:
                if not isinstance(item, dict):
                    continue
                normalized_item = _build_cookie_from_mapping(item, default_url)
                if not normalized_item:
                    continue
                explicit_scope = bool(
                    str(item.get("url") or "").strip()
                    or str(item.get("domain") or "").strip()
                )
                if explicit_scope:
                    append_cookie(normalized_item)
                    continue
                base_cookie = dict(normalized_item)
                base_cookie.pop("domain", None)
                base_cookie["path"] = str(base_cookie.get("path") or "/").strip() or "/"
                for target_url in target_urls:
                    append_cookie({**base_cookie, "url": target_url})
            return expanded

    for cookie in parse_browser_cookie_payload(normalized, default_url=default_url):
        base_cookie = dict(cookie)
        base_cookie.pop("domain", None)
        base_cookie["path"] = str(base_cookie.get("path") or "/").strip() or "/"
        for target_url in target_urls:
            append_cookie({**base_cookie, "url": target_url})
    return expanded


def _build_cookie_merge_key(cookie: Dict[str, Any], default_url: str) -> Optional[str]:
    name = str(cookie.get("name") or "").strip()
    if not name:
        return None
    domain = str(cookie.get("domain") or "").strip().lower()
    url = str(cookie.get("url") or "").strip()
    path = str(cookie.get("path") or "/").strip() or "/"
    if not domain:
        candidate_url = url or default_url
        try:
            parsed = urlparse(candidate_url)
            domain = str(parsed.hostname or "").strip().lower()
        except Exception:
            domain = ""
    return json.dumps([name, domain, path], ensure_ascii=True, separators=(",", ":"))


def merge_browser_cookie_payloads(
    base_cookie: Any,
    new_cookie_items: Any,
    default_url: str = DEFAULT_COOKIE_URL,
) -> str:
    merged: Dict[str, Dict[str, Any]] = {}

    def append_cookie_items(raw_cookie: Any):
        if raw_cookie is None:
            return
        if isinstance(raw_cookie, dict):
            payload = (
                raw_cookie.get("cookies")
                if isinstance(raw_cookie.get("cookies"), list)
                else [raw_cookie]
            )
        elif isinstance(raw_cookie, list):
            payload = raw_cookie
        else:
            payload = parse_browser_cookie_payload(raw_cookie, default_url=default_url)
        for item in payload:
            if not isinstance(item, dict):
                continue
            normalized_item = _build_cookie_from_mapping(item, default_url)
            if not normalized_item:
                continue
            merge_key = _build_cookie_merge_key(normalized_item, default_url)
            if not merge_key:
                continue
            merged[merge_key] = normalized_item

    append_cookie_items(base_cookie)
    append_cookie_items(new_cookie_items)
    if not merged:
        return ""
    return json.dumps(list(merged.values()), ensure_ascii=False, separators=(",", ":"))


def serialize_cookie_header(
    raw_cookie: Any, default_url: str = DEFAULT_COOKIE_URL
) -> str:
    normalized = normalize_cookie_storage_text(raw_cookie)
    cookies = parse_browser_cookie_payload(raw_cookie, default_url=default_url)
    if not cookies:
        if str(normalized or "").strip()[:1] in {"[", "{"}:
            return ""
        return normalize_cookie_header_text(normalized)
    parts: List[str] = []
    for cookie in cookies:
        name = str(cookie.get("name") or "").strip()
        if not name:
            continue
        parts.append(f"{name}={str(cookie.get('value') or '')}")
    return "; ".join(parts)


def _is_safe_cookie_pair(name: str, value: str) -> bool:
    if not name or not value or not _SAFE_COOKIE_NAME.fullmatch(name):
        return False
    return ";" not in value and not any(ord(character) < 0x20 for character in value)


def _cookie_path_matches(request_path: str, cookie_path: str) -> bool:
    normalized_path = cookie_path if cookie_path.startswith("/") else "/"
    if request_path == normalized_path:
        return True
    if not request_path.startswith(normalized_path):
        return False
    return normalized_path.endswith("/") or request_path[len(normalized_path) :].startswith(
        "/"
    )


def _cookie_mapping_matches_url(cookie: Dict[str, Any], target_url: str) -> bool:
    if is_partitioned_cookie(cookie):
        return False
    target = urlparse(target_url)
    target_host = str(target.hostname or "").strip().lower()
    target_path = target.path or "/"
    if not target_host:
        return False

    domain = str(cookie.get("domain") or "").strip().lower()
    host_only = bool(cookie.get("hostOnly"))
    if domain:
        normalized_domain = domain.lstrip(".")
        if host_only:
            if target_host != normalized_domain:
                return False
        elif target_host != normalized_domain and not target_host.endswith(
            f".{normalized_domain}"
        ):
            return False
    else:
        cookie_url = str(cookie.get("url") or "").strip()
        if cookie_url:
            cookie_host = str(urlparse(cookie_url).hostname or "").strip().lower()
            if cookie_host and cookie_host != target_host:
                return False

    if bool(cookie.get("secure")) and target.scheme.lower() != "https":
        return False
    if not _cookie_path_matches(target_path, str(cookie.get("path") or "/")):
        return False

    expires = cookie.get("expirationDate", cookie.get("expires"))
    if expires not in (None, "", 0, -1):
        try:
            if float(expires) <= time.time():
                return False
        except (TypeError, ValueError):
            pass

    return True


def cookie_pairs_for_url(
    raw_cookie: Any,
    target_url: str = "https://flow.google.com/projects",
) -> List[tuple[str, str]]:
    """Return the cookie pairs a browser would send to ``target_url``.

    Structured browser exports retain scope and duplicate cookie names. Plain
    Cookie headers remain supported for existing installations.
    """

    normalized = normalize_cookie_storage_text(raw_cookie)
    if not normalized:
        return []

    payload: Any = None
    if normalized[:1] in {"[", "{"}:
        try:
            payload = json.loads(normalized)
        except (TypeError, ValueError):
            payload = None

    if isinstance(payload, dict) and isinstance(payload.get("cookies"), list):
        payload = payload["cookies"]
    elif isinstance(payload, dict) and {"name", "value"}.issubset(payload):
        payload = [payload]

    if isinstance(payload, list):
        scoped: List[tuple[int, int, str, str]] = []
        for index, item in enumerate(payload):
            if not isinstance(item, dict) or not _cookie_mapping_matches_url(
                item, target_url
            ):
                continue
            name = str(item.get("name") or "").strip()
            value = str(item.get("value") or "").strip()
            if not _is_safe_cookie_pair(name, value):
                continue
            path_length = len(str(item.get("path") or "/"))
            scoped.append((path_length, index, name, value))
        scoped.sort(key=lambda record: (-record[0], record[1]))
        return [(name, value) for _, _, name, value in scoped]

    if isinstance(payload, dict):
        pairs: List[tuple[str, str]] = []
        for raw_name, raw_value in payload.items():
            name = str(raw_name or "").strip()
            value = str(raw_value or "").strip()
            if _is_safe_cookie_pair(name, value):
                pairs.append((name, value))
        return pairs

    pairs = []
    for chunk in normalized.replace("\n", ";").split(";"):
        name, separator, value = chunk.strip().partition("=")
        name = name.strip()
        value = value.strip()
        if separator and _is_safe_cookie_pair(name, value):
            pairs.append((name, value))
    return pairs


def serialize_cookie_header_for_url(
    raw_cookie: Any,
    target_url: str = "https://flow.google.com/projects",
) -> str:
    return "; ".join(
        f"{name}={value}" for name, value in cookie_pairs_for_url(raw_cookie, target_url)
    )


def validate_flow_cookie_storage(raw_cookie: Any) -> str:
    pairs = cookie_pairs_for_url(raw_cookie)
    names = {name for name, _ in pairs}
    missing: List[str] = []
    if not names.intersection(FLOW_SESSION_COOKIE_NAMES):
        missing.append("flow.google.com 会话 Cookie（OSID 或 __Secure-OSID）")
    if not names.intersection(GOOGLE_ACCOUNT_COOKIE_NAMES):
        missing.append(".google.com 账号 Cookie（SID/HSID/SSID/APISID/SAPISID）")
    if missing:
        raise ValueError("Google Cookies 不完整，缺少" + "、".join(missing))
    return "; ".join(f"{name}={value}" for name, value in pairs)


def extract_session_token_from_cookie_payload(
    raw_cookie: Any, default_url: str = DEFAULT_COOKIE_URL
) -> str:
    for cookie in parse_browser_cookie_payload(raw_cookie, default_url=default_url):
        name = str(cookie.get("name") or "").strip()
        if name not in _SESSION_TOKEN_COOKIE_NAMES:
            continue
        value = str(cookie.get("value") or "").strip()
        if not value:
            return ""
        try:
            return unquote(value)
        except Exception:
            return value
    return ""


def build_cookie_signature(
    raw_cookie: Any, default_url: str = DEFAULT_COOKIE_URL
) -> str:
    cookies = parse_browser_cookie_payload(raw_cookie, default_url=default_url)
    if not cookies:
        return ""
    stable_payload = json.dumps(
        cookies, ensure_ascii=True, sort_keys=True, separators=(",", ":")
    )
    return hashlib.sha256(stable_payload.encode("utf-8")).hexdigest()
