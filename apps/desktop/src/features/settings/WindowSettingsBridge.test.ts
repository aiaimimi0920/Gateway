import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { runInNewContext } from "node:vm";
import { expect, it, vi } from "vitest";

// Execute the shipped initialization script, not a parallel TypeScript implementation.
const native = readFileSync(resolve(process.cwd(), "src-tauri/src/connection/chrome.rs"), "utf8");
const script = native.split('INITIALIZATION_SCRIPT: &str = r#"')[1].split('"#;')[0]
  .replaceAll("__GATEWAY_SETTINGS_BASE__", "https://gateway-settings.localhost");
function bridge(fetch: ReturnType<typeof vi.fn>) {
  const navigate = vi.fn();
  const location = {};
  Object.defineProperty(location, "href", { set: navigate });
  const window = { location, dispatchEvent: vi.fn(), addEventListener: vi.fn() } as unknown as Window & { __GATEWAY_WINDOW_CHROME__: { showSettings(value: unknown): void; send(action: string): void } };
  runInNewContext(script, { window, fetch, Event, encodeURIComponent, AbortController, setTimeout, clearTimeout });
  return { window, navigate, api: window.__GATEWAY_WINDOW_CHROME__ };
}

it("sends settings requests without navigating the console or invoking native commands", async () => {
  const fetch = vi.fn().mockResolvedValue({ ok: true });
  const { api, navigate } = bridge(fetch);
  const layout = { category: "backend", x: 210, y: 160, width: 900, height: 600, theme: "dark", locale: "zh-CN" };
  api.showSettings(layout);
  await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(1));
  api.send("settings-hide");
  await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(2));
  expect(fetch.mock.calls[0]).toEqual([`https://gateway-settings.localhost/show?layout=${encodeURIComponent(JSON.stringify(layout))}`,
    { credentials: "omit", cache: "no-store", redirect: "error", signal: expect.any(AbortSignal) }]);
  expect(fetch.mock.calls[1][0]).toBe("https://gateway-settings.localhost/hide");
  expect(navigate).not.toHaveBeenCalled();
  api.send("minimize");
  expect(navigate).toHaveBeenCalledWith("gateway-window://control/minimize");
});

it("bounds resize bursts to one request in flight and the latest pending state", async () => {
  let finish!: (response: unknown) => void;
  const fetch = vi.fn().mockImplementationOnce(() => new Promise(resolve => { finish = resolve; })).mockResolvedValue({ ok: true });
  const { api, navigate } = bridge(fetch);
  api.showSettings({ category: "backend" });
  for (let x = 0; x < 100; x++) api.showSettings({ category: "desktop", x });
  api.send("settings-hide");
  expect(fetch).toHaveBeenCalledTimes(1);
  finish({ ok: true });
  await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(2));
  expect(fetch.mock.calls[1][0]).toBe("https://gateway-settings.localhost/hide");
  expect(navigate).not.toHaveBeenCalled();
});

it("reports blocked requests without falling back to a flashing navigation", async () => {
  const fetch = vi.fn().mockRejectedValue(new Error("blocked"));
  const { api, window, navigate } = bridge(fetch);
  api.showSettings({ category: "backend" });
  await vi.waitFor(() => expect(window.dispatchEvent).toHaveBeenCalledWith(expect.objectContaining({ type: "gateway:settings-error" })));
  expect(navigate).not.toHaveBeenCalled();
});
