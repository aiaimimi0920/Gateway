import { exportMaterial } from "./material.mjs";

// Runs only on an explicitly selected ChatGPT tab, in the extension's isolated world.
export async function readPageSession() {
  if (location.origin !== "https://chatgpt.com") throw new Error("wrong_origin");
  const response = await fetch("/api/auth/session", { credentials: "same-origin", redirect: "error", signal: AbortSignal.timeout(15000) });
  if (!response.ok || !response.headers.get("content-type")?.includes("application/json")) throw new Error("login_required");
  const reader = response.body.getReader();
  let size = 0;
  const chunks = [];
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      size += value.length;
      if (size > 128 * 1024) throw new Error("invalid_material");
      chunks.push(value);
    }
  } finally { await reader.cancel(); }
  const bytes = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
  const data = JSON.parse(new TextDecoder().decode(bytes));
  if (location.origin !== "https://chatgpt.com") throw new Error("wrong_origin");
  return {
    accessToken: data.accessToken ?? data.access_token, userAgent: navigator.userAgent,
    language: navigator.language, timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
    chatgptPowSources: Array.from(document.querySelectorAll("script[src]"))
      .map(node => node.getAttribute("src")).filter(src => src?.includes("/backend-api/sentinel/")).slice(0, 32),
    chatgptPowDataBuild: document.documentElement.getAttribute("data-build") || undefined,
  };
}

export async function captureSelectedTab(api) {
  const [tab] = await api.tabs.query({ active: true, currentWindow: true });
  if (!tab?.id || !tab.url || new URL(tab.url).origin !== "https://chatgpt.com") throw new Error("select_chatgpt");
  const stores = await api.cookies.getAllCookieStores();
  const store = stores.find(item => item.tabIds.includes(tab.id));
  if (!store) throw new Error("invalid_material");
  const [result] = await api.scripting.executeScript({ target: { tabId: tab.id, frameIds: [0] }, world: "ISOLATED", func: readPageSession });
  const cookies = await api.cookies.getAll({ url: "https://chatgpt.com/", storeId: store.id });
  const current = await api.tabs.get(tab.id);
  if (current.url !== tab.url) throw new Error("tab_changed");
  return exportMaterial(result?.result, cookies);
}
