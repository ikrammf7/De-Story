console.log("[Flow2API] Captcha Worker content script injected on", window.location.href);

// Install action guard immediately at document_start in MAIN world to prevent Google Flow
// from overwriting grecaptcha.enterprise.execute action with 'extension_hijack_detected'.
(() => {
    if (window.__flow2apiRecaptchaActionGuardInstalled) return;
    try {
        Object.defineProperty(window, '__flow2apiRecaptchaActionGuardInstalled', { value: true });
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
        console.log("[Flow2API] Action guard installed successfully.");
    } catch (e) {
        console.warn("[Flow2API] Failed to install action guard:", e);
    }
})();

function getRecaptchaToken(action) {
    return new Promise((resolve, reject) => {
        const siteKey = "6LdsFiUsAAAAAIjVDZcuLhaHiDn5nnHVXVRQGeMV";
        const timeoutMs = action === "VIDEO_GENERATION" ? 35000 : 25000;
        let settled = false;
        const finish = (fn, val) => {
            if (settled) return;
            settled = true;
            fn(val);
        };

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
            } catch (err) {
                clearTimeout(timer);
                finish(reject, new Error(err && err.message ? err.message : String(err)));
            }
        }

        if (typeof grecaptcha !== "undefined" && grecaptcha.enterprise && typeof grecaptcha.enterprise.execute === "function") {
            doExecute();
        } else {
            const rScript = document.createElement("script");
            rScript.src = "https://www.google.com/recaptcha/enterprise.js?render=" + siteKey;
            rScript.onload = doExecute;
            rScript.onerror = () => {
                clearTimeout(timer);
                finish(reject, new Error("Failed to load enterprise.js"));
            };
            document.head.appendChild(rScript);
        }
    });
}

chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
    if (message.type === "get_token") {
        console.log("[Flow2API] Generating token for action: " + message.action);
        getRecaptchaToken(message.action)
            .then(token => sendResponse({ status: "success", token: token }))
            .catch(err => sendResponse({ status: "error", error: err.message }));
        return true;
    }
});
