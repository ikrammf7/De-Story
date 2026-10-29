let ws = null;
let reconnectTimeout = null;
let heartbeatInterval = null;

const DEFAULT_SETTINGS = {
    serverUrl: "ws://127.0.0.1:8765/captcha_ws",
    apiKey: "",
    routeKey: "",
    clientLabel: ""
};

function getSettings() {
    return new Promise((resolve) => {
        chrome.storage.local.get(DEFAULT_SETTINGS, (stored) => {
            resolve({
                serverUrl: (stored.serverUrl || DEFAULT_SETTINGS.serverUrl).trim(),
                apiKey: (stored.apiKey || "").trim(),
                routeKey: (stored.routeKey || "").trim(),
                clientLabel: (stored.clientLabel || "").trim()
            });
        });
    });
}

function updateWsStatus(connected, errorMsg = "") {
    try {
        chrome.storage.local.set({
            wsConnected: Boolean(connected),
            wsLastError: errorMsg || "",
            wsLastUpdated: Date.now()
        });
    } catch (e) {}
}

function closeSocket() {
    if (heartbeatInterval) clearInterval(heartbeatInterval);
    heartbeatInterval = null;
    if (reconnectTimeout) clearTimeout(reconnectTimeout);
    reconnectTimeout = null;
    if (ws) {
        try {
            ws.close();
        } catch (e) {
            console.log("[Flow2API] Close socket error", e);
        }
        ws = null;
    }
    updateWsStatus(false);
}

function sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

function waitForTabReady(tabId, timeoutMs = 15000) {
    return new Promise((resolve) => {
        let settled = false;
        const finish = () => {
            if (settled) return;
            settled = true;
            chrome.tabs.onUpdated.removeListener(onUpdated);
            clearTimeout(timer);
            resolve();
        };
        const onUpdated = (updatedTabId, changeInfo) => {
            if (updatedTabId === tabId && changeInfo.status === "complete") {
                finish();
            }
        };
        const timer = setTimeout(finish, timeoutMs);

        chrome.tabs.onUpdated.addListener(onUpdated);
        chrome.tabs.get(tabId, (tab) => {
            if (chrome.runtime.lastError) {
                finish();
                return;
            }
            if (tab && tab.status === "complete") {
                finish();
            }
        });
    });
}

async function connectWS() {
    if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) return;

    const settings = await getSettings();
    let url;
    try {
        url = new URL(settings.serverUrl || DEFAULT_SETTINGS.serverUrl);
    } catch (e) {
        console.error("[Flow2API] Invalid WebSocket URL:", settings.serverUrl);
        updateWsStatus(false, "Invalid WebSocket URL: " + settings.serverUrl);
        return;
    }

    if (settings.apiKey) {
        url.searchParams.set("key", settings.apiKey);
    }
    if (settings.routeKey) {
        url.searchParams.set("route_key", settings.routeKey);
    }
    if (settings.clientLabel) {
        url.searchParams.set("client_label", settings.clientLabel);
    }

    console.log("[Flow2API] Connecting to WebSocket:", url.origin + url.pathname);
    try {
        ws = new WebSocket(url.toString());
    } catch (err) {
        console.error("[Flow2API] WebSocket constructor error:", err);
        updateWsStatus(false, err.message);
        reconnectTimeout = setTimeout(connectWS, 2000);
        return;
    }

    ws.onopen = () => {
        console.log("[Flow2API] Connected to Flow2API WebSocket successfully.");
        updateWsStatus(true);
        ws.send(JSON.stringify({
            type: "register",
            route_key: settings.routeKey,
            client_label: settings.clientLabel
        }));
        if (heartbeatInterval) clearInterval(heartbeatInterval);
        heartbeatInterval = setInterval(() => {
            if (ws && ws.readyState === WebSocket.OPEN) {
                ws.send(JSON.stringify({ type: "ping" }));
            }
        }, 20000);
    };

    let tokenQueue = Promise.resolve();

    ws.onmessage = async (event) => {
        let data;
        try {
            data = JSON.parse(event.data);
        } catch (e) {
            return;
        }

        if (data.type === "register_ack") {
            console.log("[Flow2API] Registered route key:", data.route_key || "(empty)");
            return;
        }

        if (data.type === "get_token") {
            tokenQueue = tokenQueue.then(() => handleGetToken(data)).catch(err => {
                console.error("[Flow2API] Queue Error:", err);
            });
        }
    };

    ws.onclose = (event) => {
        console.log("[Flow2API] WebSocket Closed (code " + event.code + "). Reconnecting in 2s...");
        updateWsStatus(false, event.code === 1008 ? "Connection rejected (403 Forbidden). Check API Key." : "Connection closed");
        ws = null;
        if (heartbeatInterval) clearInterval(heartbeatInterval);
        if (reconnectTimeout) clearTimeout(reconnectTimeout);
        reconnectTimeout = setTimeout(connectWS, 2000);
    };

    ws.onerror = (e) => {
        console.log("[Flow2API] WebSocket Error", e);
        updateWsStatus(false, "WebSocket communication error");
    };
}

// Injected into the page's MAIN world to generate the reCAPTCHA Enterprise token
async function recaptchaRunner(siteKey, action, timeoutMs) {
    return new Promise((resolve, reject) => {
        let settled = false;
        const finish = (fn, val) => {
            if (settled) return;
            settled = true;
            fn(val);
        };

        // Guard against Google Flow's anti-bot hijack (replacing action with extension_hijack_detected)
        try {
            if (!window.__flow2apiRecaptchaActionGuardInstalled) {
                const originalAssign = Object.assign;
                Object.assign = function(target, ...sources) {
                    let nextSources = sources;
                    if (sources.length >= 2) {
                        const previous = sources[sources.length - 2];
                        const last = sources[sources.length - 1];
                        if (
                            previous &&
                            typeof previous.action === 'string' &&
                            previous.action &&
                            last &&
                            last.action === 'extension_hijack_detected'
                        ) {
                            nextSources = sources.slice(0, -1);
                        }
                    }
                    return originalAssign.call(this, target, ...nextSources);
                };
                window.__flow2apiRecaptchaActionGuardInstalled = true;
            }
        } catch (guardErr) {
            console.warn("[Flow2API] Action guard warning:", guardErr);
        }

        const timer = setTimeout(() => {
            finish(reject, new Error("Timeout generating reCAPTCHA locally (" + timeoutMs + "ms)"));
        }, timeoutMs);

        function doExecute() {
            try {
                grecaptcha.enterprise.ready(function() {
                    let executeTarget = siteKey;
                    if (window.__flow2apiRecaptchaClientId !== undefined) {
                        executeTarget = window.__flow2apiRecaptchaClientId;
                    }
                    Promise.resolve(grecaptcha.enterprise.execute(executeTarget, { action: action }))
                        .then(token => {
                            clearTimeout(timer);
                            finish(resolve, token);
                        })
                        .catch(err => {
                            clearTimeout(timer);
                            finish(reject, new Error(err && err.message ? err.message : String(err)));
                        });
                });
            } catch (executeErr) {
                clearTimeout(timer);
                finish(reject, new Error(executeErr && executeErr.message ? executeErr.message : String(executeErr)));
            }
        }

        if (typeof grecaptcha !== "undefined" && grecaptcha.enterprise && typeof grecaptcha.enterprise.execute === "function") {
            doExecute();
        } else {
            const s = document.createElement("script");
            s.src = "https://www.google.com/recaptcha/enterprise.js?render=" + siteKey;
            s.onload = doExecute;
            s.onerror = () => {
                clearTimeout(timer);
                finish(reject, new Error("Failed to load enterprise.js via network"));
            };
            document.head.appendChild(s);
        }
    });
}

async function handleGetToken(data) {
    let tempTabId = null;
    try {
        const action = data.action || "IMAGE_GENERATION";
        const scriptTimeoutMs = action === "VIDEO_GENERATION" ? 35000 : 25000;
        const siteKey = "6LdsFiUsAAAAAIjVDZcuLhaHiDn5nnHVXVRQGeMV";

        // 1. Look for an existing open Google Flow / Labs tab
        let targetTabId = null;
        let isExistingTab = false;
        try {
            const tabs = await chrome.tabs.query({
                url: [
                    "https://flow.google.com/*",
                    "https://labs.google/*"
                ]
            });
            if (tabs && tabs.length > 0) {
                const completeTab = tabs.find(t => t.status === "complete") || tabs[0];
                targetTabId = completeTab.id;
                isExistingTab = true;
                console.log("[Flow2API] Reusing existing Google Flow tab:", targetTabId, completeTab.url);
            }
        } catch (e) {
            console.log("[Flow2API] Tab query error:", e);
        }

        // 2. If no tab is open, open a fresh background tab to https://flow.google.com/
        if (!targetTabId) {
            console.log("[Flow2API] Opening fresh background tab at https://flow.google.com/ ...");
            const newTab = await chrome.tabs.create({ url: "https://flow.google.com/", active: false });
            targetTabId = newTab.id;
            tempTabId = newTab.id;
            await waitForTabReady(targetTabId);
            await sleep(1500);
        }

        let successResponse = null;
        let lastErrorMsg = "No response from tab.";

        try {
            const results = await chrome.scripting.executeScript({
                target: { tabId: targetTabId },
                world: "MAIN",
                func: recaptchaRunner,
                args: [siteKey, action, scriptTimeoutMs]
            });

            if (results && results[0] && results[0].result) {
                successResponse = { status: "success", token: results[0].result };
            }
        } catch (e) {
            lastErrorMsg = e.message || "Script execution failed";
            // If executing on the existing tab failed, fallback to a fresh background tab
            if (isExistingTab && !tempTabId) {
                console.log("[Flow2API] Existing tab execution failed (" + lastErrorMsg + "). Falling back to fresh tab...");
                try {
                    const fallbackTab = await chrome.tabs.create({ url: "https://flow.google.com/", active: false });
                    tempTabId = fallbackTab.id;
                    await waitForTabReady(tempTabId);
                    await sleep(1500);

                    const retryResults = await chrome.scripting.executeScript({
                        target: { tabId: tempTabId },
                        world: "MAIN",
                        func: recaptchaRunner,
                        args: [siteKey, action, scriptTimeoutMs]
                    });
                    if (retryResults && retryResults[0] && retryResults[0].result) {
                        successResponse = { status: "success", token: retryResults[0].result };
                    }
                } catch (retryErr) {
                    lastErrorMsg = retryErr.message || lastErrorMsg;
                }
            }
        }

        if (successResponse && successResponse.token) {
            console.log("[Flow2API] Successfully generated reCAPTCHA token for action:", action);
            ws.send(JSON.stringify({
                req_id: data.req_id,
                status: "success",
                token: successResponse.token
            }));
        } else {
            console.error("[Flow2API] Failed to generate reCAPTCHA token:", lastErrorMsg);
            ws.send(JSON.stringify({
                req_id: data.req_id,
                status: "error",
                error: "Extension script failed: " + lastErrorMsg
            }));
        }
    } catch (err) {
        console.error("[Flow2API] handleGetToken fatal error:", err);
        ws.send(JSON.stringify({
            req_id: data.req_id,
            status: "error",
            error: err.message
        }));
    } finally {
        if (tempTabId) {
            try {
                await chrome.tabs.remove(tempTabId);
                console.log("[Flow2API] Closed temporary tab:", tempTabId);
            } catch (e) {
                console.log("[Flow2API] Error closing temporary tab:", e);
            }
        }
    }
}

chrome.storage.onChanged.addListener((changes, areaName) => {
    if (areaName !== "local") return;
    if (changes.routeKey || changes.serverUrl || changes.apiKey || changes.clientLabel) {
        console.log("[Flow2API] Extension settings changed, reconnecting WebSocket...");
        closeSocket();
        connectWS();
    }
});

connectWS();
