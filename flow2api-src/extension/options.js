const DEFAULT_SETTINGS = {
  serverUrl: "ws://127.0.0.1:8765/captcha_ws",
  apiKey: "",
  routeKey: "",
  clientLabel: ""
};

const $ = (id) => document.getElementById(id);

function normalizeSettings(values) {
  return {
    serverUrl: (values.serverUrl || DEFAULT_SETTINGS.serverUrl).trim(),
    apiKey: (values.apiKey || "").trim(),
    routeKey: (values.routeKey || "").trim(),
    clientLabel: (values.clientLabel || "").trim()
  };
}

function setStatus(message, isError = false) {
  const status = $("status");
  status.textContent = message;
  status.style.color = isError ? "#dc2626" : "#16a34a";
}

function updateConnectionBadge(connected, errorMsg = "") {
  const badge = $("connBadge");
  const text = $("connText");
  if (!badge || !text) return;

  if (connected) {
    badge.className = "badge connected";
    text.textContent = "Terhubung ke Flow2API (Connected)";
  } else {
    badge.className = "badge disconnected";
    text.textContent = errorMsg ? `Terputus: ${errorMsg}` : "Terputus dari Flow2API (Disconnected)";
  }
}

function isValidWsUrl(value) {
  try {
    const url = new URL(value);
    return url.protocol === "ws:" || url.protocol === "wss:";
  } catch (e) {
    return false;
  }
}

function loadSettings() {
  chrome.storage.local.get({ ...DEFAULT_SETTINGS, wsConnected: false, wsLastError: "" }, (stored) => {
    const settings = normalizeSettings(stored);
    $("serverUrl").value = settings.serverUrl;
    $("apiKey").value = settings.apiKey;
    $("routeKey").value = settings.routeKey;
    $("clientLabel").value = settings.clientLabel;
    updateConnectionBadge(Boolean(stored.wsConnected), stored.wsLastError);
  });
}

function saveSettings() {
  const settings = normalizeSettings({
    serverUrl: $("serverUrl").value,
    apiKey: $("apiKey").value,
    routeKey: $("routeKey").value,
    clientLabel: $("clientLabel").value
  });

  if (!isValidWsUrl(settings.serverUrl)) {
    setStatus("WebSocket URL harus diawali dengan ws:// atau wss://", true);
    return;
  }
  if (!settings.apiKey) {
    setStatus("Harap masukkan API Key Flow2API.", true);
    return;
  }

  chrome.storage.local.set(settings, () => {
    if (chrome.runtime.lastError) {
      setStatus(`Gagal menyimpan: ${chrome.runtime.lastError.message}`, true);
      return;
    }
    setStatus("Pengaturan disimpan. Ekstensi sedang menyambung ulang...");
  });
}

document.addEventListener("DOMContentLoaded", () => {
  loadSettings();
  $("saveBtn").addEventListener("click", saveSettings);

  chrome.storage.onChanged.addListener((changes, areaName) => {
    if (areaName !== "local") return;
    if (changes.wsConnected || changes.wsLastError) {
      chrome.storage.local.get({ wsConnected: false, wsLastError: "" }, (stored) => {
        updateConnectionBadge(Boolean(stored.wsConnected), stored.wsLastError);
      });
    }
  });
});
