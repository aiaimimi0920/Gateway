import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import { createBrowserHost } from "../platform/browserHost";
import { server } from "../test/server";
import { createGatewayApiClient } from "./client";
import { createConsoleApi } from "./console";

const routeConfigUrl = `${window.location.origin}/v1/internal/gateway/console/route-config`;
const accountGroupSummaryUrl = `${window.location.origin}/v1/internal/gateway/account-groups`;
const validateRouteConfigUrl = `${window.location.origin}/v1/internal/gateway/console/route-config/validate`;
const revisionsUrl = `${window.location.origin}/v1/internal/gateway/console/revisions`;
const revisionDetailUrl = `${window.location.origin}/v1/internal/gateway/console/revisions/r2-beadfeedcafe`;
const credentialProbeUrl = `${window.location.origin}/v1/internal/gateway/console/credentials/acc%2Fprod-1/probe`;

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

  it("loads the account-group summary endpoint with the management header", async () => {
    let observedToken: string | null = null;
    server.use(
      http.get(accountGroupSummaryUrl, ({ request }) => {
        observedToken = request.headers.get("x-management-token");
        return HttpResponse.json({
          summary: {
            routeConfigRevision: "r1-deadbeefcafe",
            source: "redis",
            accountGroups: [
              {
                id: "group-vip",
                name: "VIP",
                description: null,
                billingMultiplier: 1.5,
                configuredBillingMultiplier: 1.5,
                enabled: true,
                notes: null,
                memberCount: 1,
                providerCredentialIds: ["acc-prod-1"],
                providers: ["managed-provider"],
              },
            ],
            accounts: [
              {
                id: "acc-prod-1",
                displayName: "生产账号 A",
                providerId: "managed-provider",
                providerLabel: "Managed OpenAI",
                vendorKey: "muyuan",
                vendorName: "木元",
                providerPreset: "openai",
                credentialId: "acc-prod-1",
                baseUrl: "https://api.example.com/v1",
                mode: "credential",
                enabled: false,
                supportedModels: ["gpt-5.4"],
                groupIds: ["group-vip"],
              },
            ],
            providers: [
              {
                id: "managed-provider",
                label: "Managed OpenAI",
                vendorKey: "muyuan",
                vendorName: "木元",
                preset: "openai",
                baseUrl: "https://api.example.com/v1",
                accountIds: ["acc-prod-1"],
                supportedModels: ["gpt-5.4"],
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

    const response = await api.getAccountGroupSummary("management-secret");

    expect(observedToken).toBe("management-secret");
    expect(response.summary.routeConfigRevision).toBe("r1-deadbeefcafe");
    expect(response.summary.accounts[0]?.displayName).toBe("生产账号 A");
    expect(response.summary.accounts[0]?.enabled).toBe(false);
    expect(response.summary.accounts[0]?.vendorKey).toBe("muyuan");
    expect(response.summary.accounts[0]?.vendorName).toBe("木元");
    expect(response.summary.providers[0]?.vendorKey).toBe("muyuan");
    expect(response.summary.providers[0]?.vendorName).toBe("木元");
  });

  it("probes a route credential with both management and secret-grant headers", async () => {
    let observedManagementToken: string | null = null;
    let observedSecretGrant: string | null = null;
    let observedMethod: string | null = null;
    let observedBody: string | null = null;
    server.use(
      http.post(credentialProbeUrl, async ({ request }) => {
        observedManagementToken = request.headers.get("x-management-token");
        observedSecretGrant = request.headers.get("x-secret-grant");
        observedMethod = request.method;
        observedBody = await request.text();
        return HttpResponse.json({
          result: {
            credentialId: "acc/prod-1",
            providerId: "managed-provider",
            status: "passed",
            message: "Credential connectivity probe passed.",
            checkedAt: "2026-07-28T02:00:00Z",
          },
        });
      }),
    );
    const api = createConsoleApi(
      createGatewayApiClient({
        host: createBrowserHost(),
      }),
    );

    const response = await api.probeCredential(
      "management-secret",
      "grant-1",
      "acc/prod-1",
    );

    expect(observedMethod).toBe("POST");
    expect(observedManagementToken).toBe("management-secret");
    expect(observedSecretGrant).toBe("grant-1");
    expect(observedBody).toBe("");
    expect(response.result).toMatchObject({
      credentialId: "acc/prod-1",
      providerId: "managed-provider",
      status: "passed",
    });
  });

  it("validates route-config drafts through the canonical console path", async () => {
    let observedBody: unknown = null;
    let observedSecretGrant: string | null = null;
    server.use(
      http.post(validateRouteConfigUrl, async ({ request }) => {
        observedBody = await request.json();
        observedSecretGrant = request.headers.get("x-secret-grant");
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

    const response = await api.validateRouteConfig(
      "management-secret",
      {
        document: {
          providers: [],
          model_routes: [],
          aliases: { answer: "gpt-5.4" },
        },
        secretPatches: [
          {
            path: "/providers/0/api_key",
            operation: "replace",
            value: "replacement-secret",
          },
        ],
      },
      "validation-grant",
    );

    expect(observedBody).toMatchObject({
      document: {
        aliases: { answer: "gpt-5.4" },
      },
      secretPatches: [
        {
          path: "/providers/0/api_key",
          operation: "replace",
          value: "replacement-secret",
        },
      ],
    });
    expect(observedSecretGrant).toBe("validation-grant");
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

  it("loads revision details from the canonical console revisions detail path", async () => {
    let observedToken: string | null = null;
    server.use(
      http.get(revisionDetailUrl, ({ request }) => {
        observedToken = request.headers.get("x-management-token");
        return HttpResponse.json({
          routeConfig: {
            revision: { id: "r2-beadfeedcafe", sequence: 2, message: "enable gpt-5.4 route" },
            source: "archived",
            diagnostics: null,
            requiresRepair: false,
            document: {
              providers: [{ id: "managed-provider" }],
              model_routes: [{ pattern: "gpt-5.4-preview" }],
              aliases: { answer: "gpt-5.4-preview" },
            },
            secrets: [{ path: "/providers/0/api_key", configured: true, preview: "sk-***" }],
            mutationSupported: true,
          },
          active: false,
          hasArchive: true,
        });
      }),
    );
    const api = createConsoleApi(
      createGatewayApiClient({
        host: createBrowserHost(),
      }),
    );

    const response = await api.getRouteConfigRevision("management-secret", "r2-beadfeedcafe");

    expect(observedToken).toBe("management-secret");
    expect(response.routeConfig.revision.id).toBe("r2-beadfeedcafe");
    expect(response.active).toBe(false);
    expect(response.hasArchive).toBe(true);
  });

  it("commits route-config updates through the canonical console path", async () => {
    let observedBody: unknown = null;
    let observedSecretGrant: string | null = null;
    server.use(
      http.put(routeConfigUrl, async ({ request }) => {
        observedBody = await request.json();
        observedSecretGrant = request.headers.get("x-secret-grant");
        return HttpResponse.json({
          routeConfig: {
            revision: { id: "r2-beadfeedcafe", sequence: 2 },
            source: "redis",
            diagnostics: { diagnostics: [] },
            requiresRepair: false,
            document: {
              providers: [],
              model_routes: [],
              aliases: { answer: "gpt-5.4" },
            },
            secrets: [],
            mutationSupported: true,
          },
          committed: true,
        });
      }),
    );
    const api = createConsoleApi(
      createGatewayApiClient({
        host: createBrowserHost(),
      }),
    );

    const response = await api.commitRouteConfig(
      "management-secret",
      {
        expectedRevision: "r1-deadbeefcafe",
        document: {
          providers: [],
          model_routes: [],
          aliases: { answer: "gpt-5.4" },
        },
        secretPatches: [{ path: "/providers/0/api_key", operation: "clear" }],
        message: "enable gpt-5.4 route",
      },
      "commit-grant",
    );

    expect(observedBody).toMatchObject({
      expectedRevision: "r1-deadbeefcafe",
      secretPatches: [{ path: "/providers/0/api_key", operation: "clear" }],
      message: "enable gpt-5.4 route",
    });
    expect(observedSecretGrant).toBe("commit-grant");
    expect(response.routeConfig.revision.id).toBe("r2-beadfeedcafe");
  });
});
