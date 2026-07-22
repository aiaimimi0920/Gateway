import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import { server } from "../test/server";
import { fetchGatewayHealth, fetchGatewayReady, gatewayBaseUrlFromProfile } from "./http";

describe("legacy HTTP compatibility layer", () => {
  it("derives the API origin from the selected profile port", () => {
    expect(gatewayBaseUrlFromProfile({ port: 45123 })).toBe("http://127.0.0.1:45123");
  });

  it("rejects malformed successful responses through the shared Zod client", async () => {
    server.use(
      http.get("http://127.0.0.1:45123/healthz", () => HttpResponse.json({ unexpected: true })),
    );

    const result = await fetchGatewayHealth("http://127.0.0.1:45123");

    expect(result.ok).toBe(false);
    expect(result.error).toMatch(/schema/i);
  });

  it("requires the readiness status field on successful responses", async () => {
    server.use(
      http.get("http://127.0.0.1:45123/readyz", () => HttpResponse.json({ unexpected: true })),
    );

    const result = await fetchGatewayReady("http://127.0.0.1:45123");

    expect(result.ok).toBe(false);
    expect(result.error).toMatch(/schema/i);
  });
});
