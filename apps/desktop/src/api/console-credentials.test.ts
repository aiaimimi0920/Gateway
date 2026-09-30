import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import { createBrowserHost } from "../platform/browserHost";
import { server } from "../test/server";
import { createGatewayApiClient } from "./client";
import { createConsoleApi } from "./console";

const providerCredentialInventoryUrl =
  `${window.location.origin}/v1/internal/gateway/provider-credentials?maskSecrets=true`;
const credentialPoolAutomationUrl =
  `${window.location.origin}/v1/internal/gateway/credential-pool-automation`;
const credentialPoolAutomationRunUrl =
  `${credentialPoolAutomationUrl}/providers/suno%2Fweb/run`;
const credentialPoolPruneUrl =
  `${credentialPoolAutomationUrl}/providers/suno%2Fweb/prune`;
const credentialPoolArchiveUrl =
  `${credentialPoolAutomationUrl}/providers/suno%2Fweb/archive`;
const credentialRefillUrl =
  `${window.location.origin}/v1/internal/gateway/credential-pool-refill`;
const credentialRefillRequestUrl =
  `${credentialRefillUrl}/providers/managed%2Fprovider/request`;
const credentialProbeUrl = `${window.location.origin}/v1/internal/gateway/console/credentials/acc%2Fprod-1/probe`;
const providerProbeUrl = `${window.location.origin}/v1/internal/gateway/console/providers/managed%2Fprovider/probe`;
const credentialUsageUrl = `${window.location.origin}/v1/internal/gateway/usage-aggregates`;

describe("console credential API", () => {
  it("accepts a persistent local refill queue without a Redis stream", async () => {
    server.use(http.get(credentialRefillUrl, () => HttpResponse.json({
      refill: {
        enabled: true, storageBackend: "sqlite", streamKey: null,
        notificationIntervalSeconds: 30, defaultLeaseSeconds: 300,
        maxLeaseSeconds: 3600, revisionId: "local-test", providers: [], recentTasks: [],
      },
    })));
    const api = createConsoleApi(createGatewayApiClient({ host: createBrowserHost() }));
    const status = await api.getCredentialRefill("management-secret");
    expect(status.refill.storageBackend).toBe("sqlite");
    expect(status.refill.streamKey).toBeNull();
    expect(status.refill.enabled).toBe(true);
  });

  it("loads masked credential inventory with source paths and cached quota", async () => {
    let observedToken: string | null = null;
    server.use(
      http.get(providerCredentialInventoryUrl, ({ request }) => {
        observedToken = request.headers.get("x-management-token");
        return HttpResponse.json({
          credentials: [
            {
              id: "acc-free-1",
              providerAccountId: "managed-provider",
              label: "free@example.com",
              status: "active",
              credential: { email: "free@example.com", token: "[REDACTED]" },
              sourceKind: "file",
              sourcePath: "C:\\gateway\\credentials\\managed-provider\\free\\acc-free-1.json",
              syncMode: "folder",
              syncState: "synced",
              providerQuota: {
                providerAccountId: "managed-provider",
                providerCredentialId: "acc-free-1",
                providerType: "openai",
                source: "cache",
                status: "ready",
                ready: true,
                checkedAt: "2026-08-19T04:00:00Z",
                nextCheckAt: "2026-08-19T04:05:00Z",
                nextResetAt: "2026-08-20T00:00:00Z",
                planType: "free",
                representativeClaim: null,
                windows: [
                  {
                    key: "daily",
                    label: "Daily",
                    usedPercent: 25,
                    remainingRatio: 0.75,
                    limitWindowSeconds: 86400,
                    resetAt: "2026-08-20T00:00:00Z",
                    resetAfterSeconds: 3600,
                  },
                ],
                error: null,
                rawData: {},
              },
            },
          ],
        });
      }),
    );
    const api = createConsoleApi(createGatewayApiClient({ host: createBrowserHost() }));
    const getInventory = api.getProviderCredentialInventory;
    if (!getInventory) {
      throw new Error("credential inventory API is unavailable");
    }

    const response = await getInventory("management-secret");

    expect(observedToken).toBe("management-secret");
    expect(response.credentials[0]?.sourcePath).toContain("\\free\\");
    expect(response.credentials[0]?.providerQuota?.windows[0]?.remainingRatio).toBe(0.75);
  });

  it("loads and runs credential pool lifecycle actions with the management header", async () => {
    const observed: Array<{ method: string; token: string | null }> = [];
    const provider = {
      providerId: "suno/web",
      providerLabel: "Suno",
      targetSize: 3,
      credentialCount: 1,
      activeCredentialCount: 1,
      autoRefillEnabled: true,
      autoPruneEnabled: true,
      permanentDeleteEnabled: false,
      driverId: "suno-browser-import",
      driverMode: "script",
      driverConfigured: true,
      state: "succeeded",
      lastRunAt: "2026-08-14T08:00:00Z",
      nextRunAt: "2026-08-14T08:01:00Z",
      lastAction: "reconcile",
      createdCount: 2,
      prunedCount: 0,
      message: "Pool reconciled.",
      revisionId: "r46-feedfacecafe",
    };
    server.use(
      http.get(credentialPoolAutomationUrl, ({ request }) => {
        observed.push({ method: request.method, token: request.headers.get("x-management-token") });
        return HttpResponse.json({
          automation: {
            enabled: true,
            intervalSeconds: 60,
            drivers: [
              { id: "suno-browser-import", mode: "script", providerIds: ["suno/web"] },
            ],
            providers: [provider],
            revisionId: "r46-feedfacecafe",
          },
        });
      }),
      http.post(credentialPoolAutomationRunUrl, ({ request }) => {
        observed.push({ method: request.method, token: request.headers.get("x-management-token") });
        return HttpResponse.json({ provider });
      }),
      http.post(credentialPoolPruneUrl, ({ request }) => {
        observed.push({ method: request.method, token: request.headers.get("x-management-token") });
        return HttpResponse.json({ provider: { ...provider, lastAction: "prune", prunedCount: 1 } });
      }),
      http.delete(credentialPoolArchiveUrl, ({ request }) => {
        observed.push({ method: request.method, token: request.headers.get("x-management-token") });
        return HttpResponse.json({ purgedCount: 3 });
      }),
    );
    const api = createConsoleApi(createGatewayApiClient({ host: createBrowserHost() }));

    const status = await api.getCredentialPoolAutomation("management-secret");
    const run = await api.runCredentialPoolAutomation("management-secret", "suno/web");
    const prune = await api.pruneCredentialPool("management-secret", "suno/web");
    const purge = await api.purgeCredentialArchive("management-secret", "suno/web");

    expect(status.automation.providers[0]?.driverConfigured).toBe(true);
    expect(run.provider.createdCount).toBe(2);
    expect(prune.provider.lastAction).toBe("prune");
    expect(purge.purgedCount).toBe(3);
    expect(observed).toEqual([
      { method: "GET", token: "management-secret" },
      { method: "POST", token: "management-secret" },
      { method: "POST", token: "management-secret" },
      { method: "DELETE", token: "management-secret" },
    ]);
  });

  it("loads refill demand and publishes a user-requested task", async () => {
    const observed: Array<{ method: string; token: string | null; body?: unknown }> = [];
    const task = {
      id: "task-1",
      providerId: "managed/provider",
      providerLabel: "Managed Provider",
      trigger: "user_requested",
      state: "pending",
      requestedCount: 2,
      targetSize: 3,
      activeCredentialCount: 1,
      routeRevision: "r1-deadbeefcafe",
      createdAt: "2026-08-14T08:00:00Z",
      updatedAt: "2026-08-14T08:00:00Z",
      workerId: null,
      leaseUntil: null,
      attempt: 0,
      deliveryMode: null,
      createdCount: 0,
      message: null,
      revisionId: null,
    };
    server.use(
      http.get(credentialRefillUrl, ({ request }) => {
        observed.push({ method: request.method, token: request.headers.get("x-management-token") });
        return HttpResponse.json({
          refill: {
            enabled: true,
            streamKey: "gw:credential-pool:refill:requests",
            notificationIntervalSeconds: 30,
            defaultLeaseSeconds: 300,
            maxLeaseSeconds: 3_600,
            revisionId: "r1-deadbeefcafe",
            providers: [
              {
                providerId: "managed/provider",
                providerLabel: "Managed Provider",
                targetSize: 3,
                credentialCount: 1,
                activeCredentialCount: 1,
                deficit: 2,
                needsRefill: true,
                autoRefillEnabled: true,
                directDriverConfigured: false,
                notificationEnabled: true,
                inquiryEnabled: true,
                userRequestEnabled: true,
                outstandingTaskId: "task-1",
                outstandingTaskState: "pending",
                notificationApi:
                  "/v1/internal/gateway/credential-pool-refill/providers/managed_provider/tasks/claim",
                inquiryApi:
                  "/v1/internal/gateway/credential-pool-refill/providers/managed_provider",
                credentialStoragePath: "C:\\gateway\\credentials\\managed_provider",
                storagePasswordConfigured: false,
                archiveStoragePath: "C:\\gateway\\credentials\\_archive\\managed_provider",
                archivedCredentialCount: 4,
                permanentDeleteEnabled: false,
                revisionId: "r1-deadbeefcafe",
              },
            ],
            recentTasks: [task],
          },
        });
      }),
      http.post(credentialRefillRequestUrl, async ({ request }) => {
        observed.push({
          method: request.method,
          token: request.headers.get("x-management-token"),
          body: await request.json(),
        });
        return HttpResponse.json({ task, created: true });
      }),
    );
    const api = createConsoleApi(createGatewayApiClient({ host: createBrowserHost() }));

    const status = await api.getCredentialRefill("management-secret");
    const requested = await api.requestCredentialRefill(
      "management-secret",
      "managed/provider",
      2,
    );

    expect(status.refill.providers[0]?.notificationEnabled).toBe(true);
    expect(status.refill.providers[0]?.archivedCredentialCount).toBe(4);
    expect(status.refill.recentTasks[0]).not.toHaveProperty("claimToken");
    expect(requested.created).toBe(true);
    expect(requested.task.trigger).toBe("user_requested");
    expect(requested.task).not.toHaveProperty("claimToken");
    expect(observed).toEqual([
      { method: "GET", token: "management-secret" },
      { method: "POST", token: "management-secret", body: { requestedCount: 2 } },
    ]);
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
            probePoint: "GET https://managed.example/v1/models",
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
      probePoint: "GET https://managed.example/v1/models",
      status: "passed",
    });
  });

  it("probes every account for a provider with management and secret-grant headers", async () => {
    let observedManagementToken: string | null = null;
    let observedSecretGrant: string | null = null;
    server.use(
      http.post(providerProbeUrl, ({ request }) => {
        observedManagementToken = request.headers.get("x-management-token");
        observedSecretGrant = request.headers.get("x-secret-grant");
        return HttpResponse.json({
          result: {
            providerId: "managed/provider",
            status: "failed",
            message: "Provider test completed: 1 passed, 1 failed, 0 unsupported.",
            checkedAt: "2026-08-19T10:00:00Z",
            totalCount: 2,
            passedCount: 1,
            failedCount: 1,
            unsupportedCount: 0,
            results: [
              {
                credentialId: "account-a",
                providerId: "managed/provider",
                probePoint: "GET https://managed.example/v1/models",
                status: "passed",
                message: "Credential connectivity probe passed.",
                checkedAt: "2026-08-19T10:00:00Z",
              },
              {
                credentialId: "account-b",
                providerId: "managed/provider",
                probePoint: "GET https://managed.example/v1/models",
                status: "failed",
                message: "Provider probe failed with status 401 Unauthorized.",
                checkedAt: "2026-08-19T10:00:00Z",
              },
            ],
          },
        });
      }),
    );
    const api = createConsoleApi(createGatewayApiClient({ host: createBrowserHost() }));

    const response = await api.probeProvider(
      "management-secret",
      "grant-provider",
      "managed/provider",
    );

    expect(observedManagementToken).toBe("management-secret");
    expect(observedSecretGrant).toBe("grant-provider");
    expect(response.result).toMatchObject({
      providerId: "managed/provider",
      totalCount: 2,
      passedCount: 1,
      failedCount: 1,
      unsupportedCount: 0,
    });
    expect(response.result.results.map((result) => result.probePoint)).toEqual([
      "GET https://managed.example/v1/models",
      "GET https://managed.example/v1/models",
    ]);
  });

  it("loads usage aggregates for one provider credential", async () => {
    let observedToken: string | null = null;
    server.use(
      http.get(credentialUsageUrl, ({ request }) => {
        observedToken = request.headers.get("x-management-token");
        const query = new URL(request.url).searchParams;
        expect(query.get("providerCredentialRef")).toBe("acc/prod-1");
        expect(query.get("createdFrom")).toBe("2026-07-20T00:00:00Z");
        expect(query.get("limit")).toBe("1000");
        return HttpResponse.json({
          buckets: [
            {
              bucketStart: "2026-08-19T10:00:00Z",
              bucketGranularity: "hour",
              projectId: "project-a",
              userId: "user-a",
              provider: "managed-provider",
              providerCredentialRef: "acc/prod-1",
              model: "gpt-5.4",
              requestCount: 3,
              failureCount: 1,
              promptTokens: 10,
              completionTokens: 20,
              totalTokens: 30,
              cacheCreationInputTokens: 0,
              cacheReadInputTokens: 0,
              latencyMsSum: 900,
              createdAt: "2026-08-19T10:01:00Z",
              updatedAt: "2026-08-19T10:01:00Z",
            },
          ],
        });
      }),
    );
    const api = createConsoleApi(createGatewayApiClient({ host: createBrowserHost() }));
    expect(api.getCredentialUsage).toBeDefined();

    const response = await api.getCredentialUsage!(
      "management-secret",
      "acc/prod-1",
      "2026-07-20T00:00:00Z",
    );

    expect(observedToken).toBe("management-secret");
    expect(response.buckets[0]).toMatchObject({ requestCount: 3, failureCount: 1 });
  });

});
