import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import { createBrowserHost } from "../platform/browserHost";
import { server } from "../test/server";
import { createGatewayApiClient } from "./client";
import { createConsoleApi } from "./console";

const routeConfigUrl = `${window.location.origin}/v1/internal/gateway/console/route-config`;
const validateRouteConfigUrl = `${window.location.origin}/v1/internal/gateway/console/route-config/validate`;
const revisionsUrl = `${window.location.origin}/v1/internal/gateway/console/revisions`;

describe("console API", () => {
  it("loads the canonical route-config endpoint with the management header", async () => {
    let observedToken: string | null = null;
    server.use(
      http.get(routeConfigUrl, ({ request }) => {
        observedToken = request.headers.get("x-management-token");
        return HttpResponse.json({
          routeConfig: {
            revision: { id: "r1-deadbeefcafe", sequence: 1 },
            source: "redis",
            diagnostics: { diagnostics: [] },
            requiresRepair: false,
            document: {
              providers: [],
              model_routes: [],
              aliases: {},
            },
            secrets: [],
            mutationSupported: true,
          },
        });
      }),
    );
    const api = createConsoleApi(
      createGatewayApiClient({
        host: createBrowserHost(),
      }),
    );

    const response = await api.getRouteConfig("management-secret");

    expect(observedToken).toBe("management-secret");
    expect(response.routeConfig.revision.id).toBe("r1-deadbeefcafe");
    expect(response.routeConfig.source).toBe("redis");
  });

  it("validates route-config drafts through the canonical console path", async () => {
    let observedBody: unknown = null;
    server.use(
      http.post(validateRouteConfigUrl, async ({ request }) => {
        observedBody = await request.json();
        return HttpResponse.json({
          validation: {
            document: {
              providers: [],
              model_routes: [],
              aliases: { answer: "gpt-5.4" },
            },
            secrets: [],
            diagnostics: { diagnostics: [] },
            requiresRepair: false,
          },
        });
      }),
    );
    const api = createConsoleApi(
      createGatewayApiClient({
        host: createBrowserHost(),
      }),
    );

    const response = await api.validateRouteConfig("management-secret", {
      document: {
        providers: [],
        model_routes: [],
        aliases: { answer: "gpt-5.4" },
      },
      secretPatches: [],
    });

    expect(observedBody).toMatchObject({
      document: {
        aliases: { answer: "gpt-5.4" },
      },
      secretPatches: [],
    });
    expect(response.validation.document.aliases.answer).toBe("gpt-5.4");
    expect(response.validation.requiresRepair).toBe(false);
  });

  it("lists revision history from the canonical console revisions path", async () => {
    server.use(
      http.get(revisionsUrl, () =>
        HttpResponse.json({
          revisions: [
            {
              revision: {
                id: "r2-beadfeedcafe",
                sequence: 2,
                message: "enable gpt-5.4 route",
              },
              active: true,
              hasArchive: true,
              source: "redis",
            },
          ],
        }),
      ),
    );
    const api = createConsoleApi(
      createGatewayApiClient({
        host: createBrowserHost(),
      }),
    );

    const response = await api.listRouteConfigRevisions("management-secret");

    expect(response.revisions).toHaveLength(1);
    expect(response.revisions[0].revision.id).toBe("r2-beadfeedcafe");
    expect(response.revisions[0].active).toBe(true);
  });
});
