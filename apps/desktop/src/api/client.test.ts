import { http, HttpResponse } from "msw";
import { describe, expect, it, vi } from "vitest";
import { createBrowserHost } from "../platform/browserHost";
import {
  clearManagementSession,
  readManagementSessionToken,
  writeManagementSessionToken,
} from "../session/storage";
import { server } from "../test/server";
import { createGatewayApiClient } from "./client";
import { GatewayResponseValidationError } from "./errors";
import { managementSessionSchema } from "./schemas";

const verifyUrl = `${window.location.origin}/v1/internal/gateway/console/session/verify`;

describe("Gateway API client", () => {
  it("sends the management token in a header and never in the URL", async () => {
    let observedUrl = "";
    let observedToken: string | null = null;
    server.use(
      http.post(verifyUrl, ({ request }) => {
        observedUrl = request.url;
        observedToken = request.headers.get("x-management-token");
        return HttpResponse.json({
          role: "administrator",
          capabilities: ["route-config:read"],
          activeRevision: "r1-deadbeef",
          secretAccessGranted: false,
        });
      }),
    );
    const client = createGatewayApiClient({ host: createBrowserHost() });

    await client.request("/v1/internal/gateway/console/session/verify", managementSessionSchema, {
      method: "POST",
      managementToken: "management-secret",
    });

    expect(observedToken).toBe("management-secret");
    expect(observedUrl).toBe(verifyUrl);
    expect(observedUrl).not.toContain("management-secret");
  });

  it("rejects malformed successful responses with the schema diagnostics", async () => {
    server.use(
      http.post(verifyUrl, () => HttpResponse.json({ role: "administrator" })),
    );
    const client = createGatewayApiClient({ host: createBrowserHost() });

    await expect(
      client.request("/v1/internal/gateway/console/session/verify", managementSessionSchema, {
        method: "POST",
        managementToken: "management-secret",
      }),
    ).rejects.toBeInstanceOf(GatewayResponseValidationError);
  });

  it.each([401, 403])("notifies the session boundary for HTTP %s", async (status) => {
    writeManagementSessionToken("expired-token");
    const onAuthenticationFailure = vi.fn(() => clearManagementSession());
    server.use(
      http.post(verifyUrl, () =>
        HttpResponse.json(
          { error: { message: "Denied", code: "console_management_token_invalid" } },
          { status },
        ),
      ),
    );
    const client = createGatewayApiClient({
      host: createBrowserHost(),
      onAuthenticationFailure,
    });

    const request = client.request(
      "/v1/internal/gateway/console/session/verify",
      managementSessionSchema,
      { method: "POST", managementToken: "expired-token" },
    );

    await expect(request).rejects.toMatchObject({
      status,
      code: "console_management_token_invalid",
    });
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();
    expect(readManagementSessionToken()).toBeNull();
  });

  it.each([401, 403])("keeps candidate HTTP %s rejection local to the verification call", async status => {
    writeManagementSessionToken("current-token");
    const onAuthenticationFailure = vi.fn(() => clearManagementSession());
    server.use(http.post(verifyUrl, () => HttpResponse.json({ error: { message: "Rejected" } }, { status })));
    const client = createGatewayApiClient({ host: createBrowserHost(), onAuthenticationFailure });
    await expect(client.request("/v1/internal/gateway/console/session/verify", managementSessionSchema, {
      method: "POST", managementToken: "candidate-token", notifyAuthenticationFailure: false,
    })).rejects.toMatchObject({ status });
    expect(onAuthenticationFailure).not.toHaveBeenCalled();
    expect(readManagementSessionToken()).toBe("current-token");
  });

  it.each([401, 403])(
    "clears authentication before decoding non-JSON HTTP %s bodies",
    async (status) => {
      writeManagementSessionToken("expired-token");
      const onAuthenticationFailure = vi.fn(() => clearManagementSession());
      server.use(
        http.post(
          verifyUrl,
          () =>
            new HttpResponse("<html><body>Access denied</body></html>", {
              status,
              headers: { "content-type": "text/html" },
            }),
        ),
      );
      const client = createGatewayApiClient({
        host: createBrowserHost(),
        onAuthenticationFailure,
      });

      await expect(
        client.request(
          "/v1/internal/gateway/console/session/verify",
          managementSessionSchema,
          { method: "POST", managementToken: "expired-token" },
        ),
      ).rejects.toMatchObject({ name: "GatewayApiError", status });
      expect(onAuthenticationFailure).toHaveBeenCalledWith(status, "expired-token");
      expect(readManagementSessionToken()).toBeNull();
    },
  );
});
