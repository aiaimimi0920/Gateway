import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import { createBrowserHost } from "../platform/browserHost";
import { server } from "../test/server";
import { createGatewayApiClient } from "./client";
import { createConsoleApi } from "./console";

const createGeminiAuthSessionUrl =
  `${window.location.origin}/v1/internal/gateway/console/gemini-auth-sessions`;
const getGeminiAuthSessionUrl =
  `${window.location.origin}/v1/internal/gateway/console/gemini-auth-sessions/session-1`;
const completeGeminiAuthSessionUrl =
  `${window.location.origin}/v1/internal/gateway/console/gemini-auth-sessions/session-1/complete`;

describe("console Gemini auth API", () => {
  it("sends offline JSON with revision and a separate secret-grant header", async () => {
    const source = '{"schemaVersion":1,"provider":"aistudio-web","cookies":[]}';
    server.use(http.post(createGeminiAuthSessionUrl, async ({ request }) => {
      expect(request.headers.get("x-management-token")).toBe("management-secret");
      expect(request.headers.get("x-secret-grant")).toBe("import-grant");
      expect(await request.json()).toEqual({ targetFamily: "aistudio-web", providerId: "studio-pool", webSessionJson: source, expectedRevision: "r1" });
      return HttpResponse.json({ session: { id: "json-draft", targetFamily: "aistudio-web", providerId: "studio-pool",
        status: "succeeded", message: "Offline draft only", createdAt: "now", updatedAt: "now", generatedDrafts: [] } });
    }));
    const api = createConsoleApi(createGatewayApiClient({ host: createBrowserHost() }));
    await api.createGeminiAuthSession("management-secret", { targetFamily: "aistudio-web", providerId: "studio-pool", webSessionJson: source, expectedRevision: "r1" }, "import-grant");
  });
  it("supports AI Studio on the existing managed auth contract without returning cookies", async () => {
    let observedBody: unknown;
    server.use(http.post(createGeminiAuthSessionUrl, async ({ request }) => {
      expect(request.headers.get("x-management-token")).toBe("management-secret");
      observedBody = await request.json();
      return HttpResponse.json({ session: { id: "aistudio-session", targetFamily: "aistudio-web", providerId: "renamed-pool",
        status: "waiting_user", message: "Close the dedicated window after signing in.",
        createdAt: "2026-11-01T00:00:00Z", updatedAt: "2026-11-01T00:00:00Z", generatedDrafts: [] } });
    }));
    const api = createConsoleApi(createGatewayApiClient({ host: createBrowserHost() }));
    const response = await api.createGeminiAuthSession("management-secret", {
      targetFamily: "aistudio-web", providerId: "renamed-pool", accountLabel: "My AI Studio",
    });
    expect(observedBody).toEqual({ targetFamily: "aistudio-web", providerId: "renamed-pool", accountLabel: "My AI Studio" });
    expect(response.session.targetFamily).toBe("aistudio-web");
    expect(response.session).not.toHaveProperty("bundle");
  });

  it("requests manual Gemini auth completion through the canonical console path", async () => {
    let observedManagementToken: string | null = null;
    let observedMethod: string | null = null;
    server.use(
      http.post(completeGeminiAuthSessionUrl, ({ request }) => {
        observedManagementToken = request.headers.get("x-management-token");
        observedMethod = request.method;
        return HttpResponse.json({
          session: {
            id: "session-1",
            targetFamily: "gemini-canvas",
            providerId: "gemini-canvas",
            status: "waiting_user",
            message: "Manual Gemini import requested. Finishing capture.",
            createdAt: "2026-07-30T12:00:00Z",
            updatedAt: "2026-07-30T12:00:02Z",
            generatedDrafts: [],
          },
        });
      }),
    );
    const api = createConsoleApi(
      createGatewayApiClient({
        host: createBrowserHost(),
      }),
    );

    const response = await api.completeGeminiAuthSession("management-secret", "session-1");

    expect(observedMethod).toBe("POST");
    expect(observedManagementToken).toBe("management-secret");
    expect(response.session.message).toContain("Finishing capture");
    expect(response.session.id).toBe("session-1");
  });

  it("creates a Gemini auth session through the canonical console path", async () => {
    let observedToken: string | null = null;
    let observedMethod: string | null = null;
    let observedBody: unknown = null;
    server.use(
      http.post(createGeminiAuthSessionUrl, async ({ request }) => {
        observedToken = request.headers.get("x-management-token");
        observedMethod = request.method;
        observedBody = await request.json();
        return HttpResponse.json({
          session: {
            id: "session-1",
            targetFamily: "gemini-canvas",
            providerId: "gemini-canvas",
            status: "waiting_user",
            message: "Complete Gemini login in the opened browser window.",
            createdAt: "2026-07-30T09:00:00Z",
            updatedAt: "2026-07-30T09:00:00Z",
            generatedDrafts: [],
          },
        });
      }),
    );
    const api = createConsoleApi(
      createGatewayApiClient({
        host: createBrowserHost(),
      }),
    );

    const response = await api.createGeminiAuthSession("management-secret", {
      targetFamily: "gemini-canvas",
      providerId: "gemini-canvas",
    });

    expect(observedMethod).toBe("POST");
    expect(observedToken).toBe("management-secret");
    expect(observedBody).toMatchObject({
      targetFamily: "gemini-canvas",
      providerId: "gemini-canvas",
    });
    expect(response.session.status).toBe("waiting_user");
    expect(response.session.targetFamily).toBe("gemini-canvas");
  });

  it("loads a Gemini auth session through the canonical console path", async () => {
    let observedToken: string | null = null;
    server.use(
      http.get(getGeminiAuthSessionUrl, ({ request }) => {
        observedToken = request.headers.get("x-management-token");
        return HttpResponse.json({
          session: {
            id: "session-1",
            targetFamily: "gemini-canvas",
            providerId: "gemini-canvas",
            status: "succeeded",
            message: "Gemini Canvas runtime captured.",
            createdAt: "2026-07-30T09:00:00Z",
            updatedAt: "2026-07-30T09:02:00Z",
            generatedDrafts: [
              {
                providerId: "gemini-canvas",
                credential: {
                  id: "gemini-canvas-manual-1",
                  account_name: "Gemini Canvas Manual 1",
                  runtime_state_object_key:
                    "credential-runtime/gemini-canvas/manual-1/storage-state.json",
                  extra_body: {
                    shareId: "fe24c455a570",
                  },
                },
                secretEdits: [],
              },
              {
                providerId: "gemini-canvas-chat",
                credential: {
                  id: "gemini-canvas-chat-manual-1",
                  account_name: "Gemini Canvas Chat Manual 1",
                  runtime_state_object_key:
                    "credential-runtime/gemini-canvas/manual-1/storage-state.json",
                  extra_body: {
                    shareId: "fe24c455a570",
                    apiBaseUrl: "https://generativelanguage.googleapis.com/v1beta",
                  },
                },
                secretEdits: [],
              },
            ],
          },
        });
      }),
    );
    const api = createConsoleApi(
      createGatewayApiClient({
        host: createBrowserHost(),
      }),
    );

    const response = await api.getGeminiAuthSession("management-secret", "session-1");

    expect(observedToken).toBe("management-secret");
    expect(response.session.status).toBe("succeeded");
    expect(response.session.generatedDrafts).toHaveLength(2);
    expect(response.session.generatedDrafts[1]?.providerId).toBe("gemini-canvas-chat");
  });

});
