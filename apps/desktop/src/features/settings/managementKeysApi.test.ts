import { afterEach, expect, it, vi } from "vitest";
import { createBrowserHost } from "../../platform/browserHost";
import { managementKeysApi } from "./managementKeysApi";

afterEach(() => vi.unstubAllGlobals());

it("uses authenticated same-origin requests and keeps new secrets out of URLs", async () => {
  const fetch = vi.fn().mockResolvedValue(new Response(JSON.stringify({ keys: [
    { id: "initial", name: "Initial", createdAt: "", current: true },
  ] }), { status: 200, headers: { "Content-Type": "application/json" } }));
  vi.stubGlobal("fetch", fetch);
  const host = createBrowserHost();
  const api = managementKeysApi(host);
  expect((await api.list("test-current")).keys).toHaveLength(1);
  fetch.mockResolvedValueOnce(new Response('{"success":true}', { status: 200 }));
  await api.add("test-current", "New", "test-new");
  const [url, options] = fetch.mock.calls[1];
  expect(url).toBe(`${host.apiOrigin}/v1/internal/gateway/console/management-keys`);
  expect(url).not.toContain("test-new");
  expect(options.headers.get("x-management-token")).toBe("test-current");
  expect(JSON.parse(options.body)).toEqual({ action: "add", name: "New", token: "test-new" });
  fetch.mockResolvedValueOnce(new Response('{"success":true}', { status: 200 }));
  await api.revoke("test-current", "initial");
  expect(JSON.parse(fetch.mock.calls[2][1].body)).toEqual({ action: "revoke", id: "initial" });
  fetch.mockResolvedValueOnce(new Response('{"success":true}', { status: 200 }));
  await api.edit("test-current", "initial", "Renamed", "replacement");
  expect(JSON.parse(fetch.mock.calls[3][1].body)).toEqual({ action: "edit", id: "initial", name: "Renamed", token: "replacement" });
  fetch.mockResolvedValueOnce(new Response('{"token":"revealed"}', { status: 200 }));
  expect((await api.reveal("test-current", "initial")).token).toBe("revealed");
  expect(JSON.parse(fetch.mock.calls[4][1].body)).toEqual({ action: "reveal", id: "initial" });
  expect(fetch.mock.calls.every(([url]) => !url.includes("replacement") && !url.includes("test-current"))).toBe(true);
});
