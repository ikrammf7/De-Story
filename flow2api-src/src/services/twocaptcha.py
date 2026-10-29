"""2Captcha API v2 integration for reCAPTCHA v3 and Enterprise."""

import asyncio
import re
from typing import Any, Dict, Optional

from curl_cffi.requests import AsyncSession


DEFAULT_BASE_URL = "https://api.2captcha.com"


class TwoCaptchaError(RuntimeError):
    """A provider failure safe to include in application diagnostics."""


def _response_payload(response, client_key: str) -> Dict[str, Any]:
    if response.status_code != 200:
        raise TwoCaptchaError(f"2Captcha API HTTP {response.status_code}")
    try:
        payload = response.json()
    except Exception:
        raise TwoCaptchaError("2Captcha API 返回无效 JSON") from None
    if not isinstance(payload, dict):
        raise TwoCaptchaError("2Captcha API 返回格式无效")
    if payload.get("errorId", 0) != 0:
        code = str(payload.get("errorCode") or "")
        if not re.fullmatch(r"ERROR_[A-Z0-9_]{1,64}", code) or client_key in code:
            code = "UNKNOWN_ERROR"
        # Provider descriptions and raw payloads may echo credentials.
        raise TwoCaptchaError(f"2Captcha: {code}")
    return payload


async def solve_recaptcha_v3(
    client_key: str,
    website_url: str,
    website_key: str,
    action: str,
    *,
    enterprise: bool = True,
    base_url: str = DEFAULT_BASE_URL,
    proxy_url: Optional[str] = None,
    timeout: float = 180.0,
) -> Dict[str, str]:
    """Return a solved token with a bounded wait and the documented polling interval."""
    if not str(client_key or "").strip():
        raise TwoCaptchaError("2Captcha API Key 未配置")
    if not str(action or "").strip():
        raise TwoCaptchaError("2Captcha reCAPTCHA action 不能为空")

    async def solve() -> Dict[str, str]:
        options = {"timeout": 30, "allow_redirects": False}
        async with AsyncSession(
            impersonate="chrome", trust_env=False, proxy=proxy_url or None
        ) as session:
            response = await session.post(
                f"{base_url.rstrip('/')}/createTask",
                json={
                    "clientKey": client_key,
                    "task": {
                        "type": "RecaptchaV3TaskProxyless",
                        "websiteURL": website_url,
                        "websiteKey": website_key,
                        "minScore": 0.9,
                        "pageAction": action,
                        "isEnterprise": bool(enterprise),
                    },
                },
                **options,
            )
            payload = _response_payload(response, client_key)
            task_id = payload.get("taskId")
            if (
                not isinstance(task_id, int)
                or isinstance(task_id, bool)
                or task_id <= 0
            ):
                raise TwoCaptchaError("2Captcha 创建任务响应缺少有效 taskId")
            while True:
                await asyncio.sleep(5)
                response = await session.post(
                    f"{base_url.rstrip('/')}/getTaskResult",
                    json={"clientKey": client_key, "taskId": task_id},
                    **options,
                )
                payload = _response_payload(response, client_key)
                if payload.get("status") == "processing":
                    continue
                if payload.get("status") != "ready":
                    raise TwoCaptchaError("2Captcha 返回未知任务状态")
                solution = payload.get("solution")
                if not isinstance(solution, dict):
                    raise TwoCaptchaError("2Captcha 返回结果缺少验证码")
                token = solution.get("gRecaptchaResponse") or solution.get("token")
                if not isinstance(token, str) or not token.strip():
                    raise TwoCaptchaError("2Captcha 返回结果缺少验证码")
                user_agent = solution.get("userAgent")
                return {
                    "gRecaptchaResponse": token,
                    "userAgent": user_agent if isinstance(user_agent, str) else "",
                }

    try:
        return await asyncio.wait_for(solve(), timeout=timeout)
    except TwoCaptchaError:
        raise
    except asyncio.TimeoutError:
        raise TwoCaptchaError("2Captcha 获取验证码超时") from None
    except Exception as exc:
        raise TwoCaptchaError(f"2Captcha API 请求失败 ({type(exc).__name__})") from None
