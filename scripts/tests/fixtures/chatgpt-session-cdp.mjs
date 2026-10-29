// Synthetic same-origin responses inside an owned test Chrome; no real account or upstream traffic.
import assert from "node:assert/strict";

export async function serveChatgptFixture(port, origin, { challenge = false, delayFirst = true } = {}) {
  const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  const target = targets.find((entry) => entry.type === "page" && entry.url.startsWith(origin));
  assert.ok(target, "owned loopback fixture page exists");
  assert.ok(target.webSocketDebuggerUrl.startsWith(`ws://127.0.0.1:${port}/devtools/page/`));
  const socket = new WebSocket(target.webSocketDebuggerUrl);
  const pending = new Map();
  const timers = new Set();
  let nextId = 0;
  const stats = { sessionRequests: 0, catalogRequests: 0, fulfilled: 0 };
  const token = `e30.${Buffer.from(JSON.stringify({ exp: 4070908800 })).toString("base64url")}.synthetic`;
  const command = (method, params = {}) => new Promise((resolve, reject) => {
    const id = ++nextId;
    const timer = setTimeout(() => { pending.delete(id); reject(new Error("fixture_command_timeout")); }, 5000);
    pending.set(id, { resolve, reject, timer });
    socket.send(JSON.stringify({ id, method, params }));
  });
  const disconnect = () => {
    for (const item of pending.values()) { clearTimeout(item.timer); item.reject(new Error("fixture_disconnected")); }
    pending.clear();
  };
  socket.addEventListener("close", disconnect);
  socket.addEventListener("error", disconnect);
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    const item = pending.get(message.id);
    if (item) {
      pending.delete(message.id); clearTimeout(item.timer);
      if (message.error) item.reject(new Error("fixture_command_failed")); else item.resolve(message.result);
    }
    if (message.method !== "Fetch.requestPaused") return;
    const { requestId, request } = message.params;
    const session = request.url === "https://chatgpt.com/api/auth/session";
    const catalog = request.url === "https://chatgpt.com/backend-api/models?history_and_training_disabled=false";
    if (session) stats.sessionRequests++;
    if (catalog) stats.catalogRequests++;
    const body = session ? JSON.stringify({ accessToken: token, user: { email: "synthetic@example.test" } })
      : catalog ? JSON.stringify({ models: [{ slug: "synthetic-model", name: "Display name" }] })
      : "<!doctype html><html data-build='fixture'><body>synthetic ChatGPT session fixture</body></html>";
    const fulfill = () => command("Fetch.fulfillRequest", { requestId, responseCode: session && challenge ? 403 : 200,
      responseHeaders: [{ name: "content-type", value: session || catalog ? "application/json" : "text/html; charset=utf-8" },
        ...(session && challenge ? [{ name: "cf-mitigated", value: "challenge" }] : [])],
      body: Buffer.from(body).toString("base64"),
    }).then(() => { stats.fulfilled++; }).catch(() => {});
    // Force the first real Runtime.evaluate to outlive the injected CDP test deadline.
    if (session && stats.sessionRequests === 1 && delayFirst) {
      const timer = setTimeout(() => { timers.delete(timer); void fulfill(); }, 650);
      timers.add(timer);
    } else { void fulfill(); }
  });
  const dispose = () => {
    for (const timer of timers) clearTimeout(timer);
    timers.clear(); disconnect(); socket.close();
  };
  let parked = false;
  const park = async () => {
    if (parked) return;
    // Detach the fixture's extra CDP owner without resuming any real first-party request.
    await command("Page.navigate", { url: "about:blank" });
    const state = await command("Runtime.evaluate", { expression: "location.href", returnByValue: true });
    assert.equal(state.result?.value, "about:blank");
    parked = true;
    const closed = new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error("fixture_disconnect_timeout")), 2000);
      socket.addEventListener("close", () => { clearTimeout(timer); resolve(); }, { once: true });
    });
    dispose(); await closed;
  };
  try {
    await new Promise((resolve, reject) => {
      socket.addEventListener("open", resolve, { once: true });
      socket.addEventListener("error", reject, { once: true });
    });
    // Intercept every request in this page before navigation: never contact real ChatGPT.
    await command("Fetch.enable", { patterns: [{ urlPattern: "*", requestStage: "Request" }] });
    await command("Network.setCookie", { name: "oai-did", value: "synthetic-device", url: "https://chatgpt.com/", secure: true });
    await command("Network.setCookie", { name: "synthetic-session", value: "fixture", url: "https://chatgpt.com/", httpOnly: true, secure: true });
    await command("Page.navigate", { url: "https://chatgpt.com/" });
    for (let attempt = 0; attempt < 50; attempt++) {
      const state = await command("Runtime.evaluate", { expression: "location.origin + ':' + document.readyState", returnByValue: true });
      if (state.result?.value === "https://chatgpt.com:complete") return { stats, token, dispose, park };
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    throw new Error("fixture_document_not_ready");
  } catch (error) { dispose(); throw error; }
}
