import { afterEach, expect, it, vi } from "vitest";
import { createBrowserHost } from "../../platform/browserHost";
import { gatewayDiagnosticsApi } from "./gatewayDiagnosticsApi";

afterEach(() => { vi.restoreAllMocks(); vi.useRealTimers(); });
it("uses only the selected backend and keeps management tokens out of URLs and redirects", async () => {
  const fetch = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(JSON.stringify({
    logDirectory: "/backend/logs", retentionDays: 7, logLimitMb: 100,
  })));
  const api = gatewayDiagnosticsApi({ ...createBrowserHost(), apiOrigin: "https://selected.example" });
  const signal = new AbortController().signal;
  await api.save("private-management-token", { retentionDays: 7, logLimitMb: 100 }, signal);
  const [url, options] = fetch.mock.calls[0];
  expect(url).toBe("https://selected.example/v1/internal/gateway/console/diagnostics/settings");
  expect(url).not.toContain("private-management-token");
  expect(options).toMatchObject({ method: "PUT", credentials: "omit", redirect: "error", cache: "no-store" });
  expect(options?.signal?.aborted).toBe(false);
  expect(new Headers(options?.headers).get("x-management-token")).toBe("private-management-token");
});
it("cancels disconnected backends after a bounded timeout and propagates owner cancellation", async () => {
  vi.useFakeTimers();
  const fetch = vi.spyOn(globalThis, "fetch").mockImplementation((_url, options) => new Promise((_resolve, reject) => {
    options?.signal?.addEventListener("abort", () => reject(new DOMException("Aborted", "AbortError")), { once: true });
  }));
  const api = gatewayDiagnosticsApi(createBrowserHost());
  const owner = new AbortController();
  const timed = expect(api.load("token", owner.signal)).rejects.toMatchObject({ name: "AbortError" });
  await vi.advanceTimersByTimeAsync(30_000);
  await timed;
  expect(owner.signal.aborted).toBe(false);
  const cancelled = expect(api.load("token", owner.signal)).rejects.toMatchObject({ name: "AbortError" });
  owner.abort();
  await cancelled;
  expect(fetch.mock.calls[1][1]?.signal?.aborted).toBe(true);
  expect(vi.getTimerCount()).toBe(0);
});
it("rejects unsupported downloads rather than saving an executable or arbitrary filename", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(JSON.stringify({
    fileName: "unsafe.exe", mimeType: "application/octet-stream", content: "payload", truncated: false,
  })));
  await expect(gatewayDiagnosticsApi(createBrowserHost()).export("token", "logs", new AbortController().signal)).rejects.toThrow();
});
