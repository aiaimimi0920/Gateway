import { expect, it, vi } from "vitest";
import { z } from "zod";
import { createGatewayApiClient } from "./client";
import { createBrowserHost } from "../platform/browserHost";

it("sends binary FormData with browser boundary and preserves management transport constraints", async () => {
  const fetchImplementation = vi.fn(async () => new Response('{"ok":true}', { headers: { "content-type": "application/json" } }));
  const client = createGatewayApiClient({ host: createBrowserHost(), fetchImplementation: fetchImplementation as typeof fetch });
  const body = new FormData();
  body.append("metadata", '{"size_bytes":3}');
  body.append("file", new File([new Uint8Array([1, 2, 3])], "test.png", { type: "image/png" }));
  await client.request("/v1/internal/gateway/console/luma-agents-files", z.object({ ok: z.boolean() }), {
    method: "POST", managementToken: "owner", secretGrant: "grant", body, headers: { "content-type": "application/json" },
  });
  const [url, init] = (fetchImplementation.mock.calls as unknown as [string, RequestInit][])[0];
  expect(new URL(url).origin).toBe(window.location.origin);
  expect(init.body).toBe(body);
  const headers = new Headers(init.headers);
  expect(headers.has("content-type")).toBe(false);
  expect(headers.get("x-management-token")).toBe("owner");
  expect(headers.get("x-secret-grant")).toBe("grant");
  expect(init).toMatchObject({ redirect: "error", credentials: "omit", cache: "no-store", referrerPolicy: "no-referrer" });
});
