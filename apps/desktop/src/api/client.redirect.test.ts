import { http, HttpResponse } from "msw";
import { describe, expect, it, vi } from "vitest";
import { z } from "zod";
import { createBrowserHost } from "../platform/browserHost";
import { server } from "../test/server";
import { createGatewayApiClient } from "./client";

describe("Gateway API redirect boundary", () => {
  it.each([301, 302, 303, 307, 308])(
    "does not forward management credentials through an HTTP %s redirect",
    async (status) => {
      const redirected = vi.fn(() => HttpResponse.json({ ok: true }));
      server.use(
        http.get(`${window.location.origin}/redirect`, () =>
          new HttpResponse(null, {
            status,
            headers: { Location: "https://other-gateway.test/destination" },
          }),
        ),
        http.get("https://other-gateway.test/destination", redirected),
      );
      const client = createGatewayApiClient({ host: createBrowserHost() });

      await expect(client.request("/redirect", z.object({ ok: z.boolean() }), {
        managementToken: "test-management-token",
        secretGrant: "test-secret-grant",
        redirect: "follow",
      })).rejects.toThrow();
      expect(redirected).not.toHaveBeenCalled();
    },
  );

  it("keeps ordinary JSON requests working with the enforced redirect policy", async () => {
    const fetchImplementation = vi.fn(async () => Response.json({ ok: true }));
    const client = createGatewayApiClient({ host: createBrowserHost(), fetchImplementation });

    await expect(client.request("/direct", z.object({ ok: z.boolean() }))).resolves.toEqual({ ok: true });
    expect(fetchImplementation).toHaveBeenCalledWith(
      `${window.location.origin}/direct`,
      expect.objectContaining({ redirect: "error", credentials: "omit", cache: "no-store" }),
    );
  });
});
