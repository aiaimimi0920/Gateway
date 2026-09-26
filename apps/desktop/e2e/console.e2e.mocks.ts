import type { Page } from "@playwright/test";
import type {
  ConsoleRouteConfigCommitRequest,
  ConsoleRouteConfigResponse,
  ConsoleRouteRevisionDetailResponse,
  ConsoleRouteRevisionListResponse,
  ManagementSession,
} from "../src/api/contracts";

export type MockConsoleState = {
  bootstrapRequired: boolean;
  routeConfig: ConsoleRouteConfigResponse;
  revisions: ConsoleRouteRevisionListResponse;
  lastCommitRequest: ConsoleRouteConfigCommitRequest | null;
  bootstrappedTokens: string[];
  verifiedTokens: string[];
};

function createRouteConfigResponse(revisionId = "r1-deadbeefcafe"): ConsoleRouteConfigResponse {
  return {
    routeConfig: {
      revision: { id: revisionId, sequence: 1, message: "initial import" },
      source: "redis",
      diagnostics: { diagnostics: [] },
      requiresRepair: false,
      document: {
        providers: [
          {
            id: "managed-provider",
            preset: "openai",
            base_url: "https://api.primary.example.com",
            supported_models: ["gpt-5.4"],
          },
        ],
        model_routes: [{ pattern: "gpt-5.4" }],
        aliases: { answer: "gpt-5.4" },
      },
      secrets: [
        { path: "/providers/0/api_key", configured: true, preview: "sk-***" },
        {
          path: "/providers/0/credential_storage_password",
          configured: true,
          preview: "sto***ord",
        },
      ],
      mutationSupported: true,
    },
  };
}

function createRevisionListResponse(): ConsoleRouteRevisionListResponse {
  return {
    revisions: [
      {
        revision: {
          id: "r1-deadbeefcafe",
          sequence: 1,
          message: "initial import",
        },
        active: true,
        hasArchive: true,
        source: "redis",
      },
      {
        revision: {
          id: "r0-cafebabefeed",
          sequence: 0,
          message: "seed route",
        },
        active: false,
        hasArchive: true,
        source: "archived",
      },
    ],
  };
}

function createRevisionDetailResponse(): ConsoleRouteRevisionDetailResponse {
  return {
    routeConfig: {
      revision: {
        id: "r0-cafebabefeed",
        sequence: 0,
        message: "seed route",
      },
      source: "archived",
      diagnostics: null,
      requiresRepair: false,
      document: {
        providers: [
          {
            id: "legacy-provider",
            preset: "openai",
            base_url: "https://api.legacy.example.com",
            supported_models: ["gpt-4.1"],
          },
        ],
        model_routes: [{ pattern: "gpt-4.1" }],
        aliases: { answer: "gpt-4.1" },
      },
      secrets: [
        { path: "/providers/0/api_key", configured: true, preview: "sk-***" },
        {
          path: "/providers/0/credential_storage_password",
          configured: true,
          preview: "sto***ord",
        },
      ],
      mutationSupported: true,
    },
    active: false,
    hasArchive: true,
  };
}

function createManagementSession(activeRevision = "r1-deadbeefcafe"): ManagementSession {
  return {
    role: "administrator",
    capabilities: ["route-config:read", "route-config:write", "route-config:revisions"],
    activeRevision,
    secretAccessGranted: false,
  };
}

function createMockConsoleState(bootstrapRequired = false): MockConsoleState {
  return {
    bootstrapRequired,
    routeConfig: createRouteConfigResponse(),
    revisions: createRevisionListResponse(),
    lastCommitRequest: null,
    bootstrappedTokens: [],
    verifiedTokens: [],
  };
}

export async function installConsoleApiMocks(page: Page, bootstrapRequired = false): Promise<MockConsoleState> {
  const state = createMockConsoleState(bootstrapRequired);

  await page.route("**/v1/internal/gateway/console/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    const path = url.pathname;

    if (path.endsWith("/bootstrap/status")) {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          needsBootstrap: state.bootstrapRequired,
          managementConfigured: !state.bootstrapRequired,
          environmentOverride: false,
        }),
      });
      return;
    }

    if (path.endsWith("/bootstrap") && request.method() === "POST") {
      const body = request.postDataJSON() as { token: string };
      state.bootstrapRequired = false;
      state.bootstrappedTokens.push(body.token);
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ success: true }),
      });
      return;
    }

    if (path.endsWith("/session/verify") && request.method() === "POST") {
      const managementToken = request.headers()["x-management-token"];
      state.verifiedTokens.push(managementToken ?? "");
      if (!managementToken || managementToken.trim().length === 0) {
        await route.fulfill({
          status: 401,
          contentType: "application/json",
          body: JSON.stringify({
            error: { message: "Missing management token.", type: "auth_error", code: "missing_token" },
          }),
        });
        return;
      }
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(createManagementSession(state.routeConfig.routeConfig.revision.id)),
      });
      return;
    }

    if (path.endsWith("/session/logout") && request.method() === "POST") {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ success: true }),
      });
      return;
    }

    if (path.endsWith("/route-config") && request.method() === "GET") {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(state.routeConfig),
      });
      return;
    }

    if (path.endsWith("/route-config/validate") && request.method() === "POST") {
      const body = request.postDataJSON() as ConsoleRouteConfigCommitRequest;
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          validation: {
            document: body.document,
            secrets: state.routeConfig.routeConfig.secrets,
            diagnostics: { diagnostics: [] },
            requiresRepair: false,
          },
        }),
      });
      return;
    }

    if (path.endsWith("/route-config") && request.method() === "PUT") {
      const body = request.postDataJSON() as ConsoleRouteConfigCommitRequest;
      state.lastCommitRequest = body;
      state.routeConfig = {
        routeConfig: {
          ...state.routeConfig.routeConfig,
          revision: {
            id: "r2-beadfeedcafe",
            sequence: 2,
            message: body.message ?? "save update",
          },
          document: body.document,
        },
      };
      state.revisions = {
        revisions: [
          {
            revision: state.routeConfig.routeConfig.revision,
            active: true,
            hasArchive: true,
            source: "redis",
          },
          ...state.revisions.revisions.map((entry, index) => ({
            ...entry,
            active: false,
            source: index === 0 ? "archived" : entry.source,
          })),
        ],
      };
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          routeConfig: state.routeConfig.routeConfig,
          committed: true,
        }),
      });
      return;
    }

    if (path.endsWith("/revisions") && request.method() === "GET") {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(state.revisions),
      });
      return;
    }

    if (path.includes("/revisions/") && request.method() === "GET") {
      const revisionId = decodeURIComponent(path.split("/").pop() ?? "");
      if (revisionId !== "r0-cafebabefeed") {
        await route.fulfill({
          status: 404,
          contentType: "application/json",
          body: JSON.stringify({
            error: { message: `Revision not found: ${revisionId}`, type: "not_found", code: "revision_missing" },
          }),
        });
        return;
      }

      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(createRevisionDetailResponse()),
      });
      return;
    }

    await route.continue();
  });

  await page.route("**/v1/internal/gateway/account-groups", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        summary: {
          routeConfigRevision: "r1-deadbeefcafe",
          source: "e2e",
          accountGroups: [],
          accounts: [
            {
              id: "managed-provider-account",
              displayName: "Managed Provider Account",
              providerId: "managed-provider",
              providerLabel: "managed-provider",
              providerPreset: "openai",
              credentialId: null,
              baseUrl: "https://api.primary.example.com",
              mode: "managed",
              enabled: true,
              supportedModels: ["gpt-5.4"],
              groupIds: [],
            },
            {
              id: "longcat:longcat-live",
              displayName: "LongCat Current",
              providerId: "longcat",
              providerLabel: "LongCat",
              vendorName: "LongCat",
              providerPreset: "longcat-openai",
              credentialId: "longcat-live",
              baseUrl: "https://api.longcat.chat/openai",
              mode: "credential",
              enabled: true,
              supportedModels: ["LongCat-2.0"],
              groupIds: [],
            },
          ],
          providers: [
            {
              id: "managed-provider",
              label: "managed-provider",
              preset: "openai",
              baseUrl: "https://api.primary.example.com",
              accountIds: ["managed-provider-account"],
              supportedModels: ["gpt-5.4"],
            },
            {
              id: "longcat",
              label: "LongCat",
              vendorName: "LongCat",
              preset: "longcat-openai",
              baseUrl: "https://api.longcat.chat/openai",
              accountIds: ["longcat:longcat-live"],
              supportedModels: ["LongCat-2.0"],
            },
          ],
        },
      }),
    });
  });

  await page.route("**/v1/internal/gateway/provider-credentials*", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ credentials: [] }),
    });
  });

  await page.route("**/v1/internal/gateway/credential-pool-automation", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        automation: {
          enabled: true,
          intervalSeconds: 60,
          drivers: [],
          providers: [
            {
              providerId: "managed-provider",
              providerLabel: "managed-provider",
              targetSize: 30,
              credentialCount: 1,
              activeCredentialCount: 1,
              autoRefillEnabled: false,
              autoPruneEnabled: false,
              permanentDeleteEnabled: false,
              driverId: null,
              driverMode: null,
              driverConfigured: false,
              state: "not_configured",
              lastRunAt: null,
              nextRunAt: null,
              lastAction: null,
              createdCount: 0,
              prunedCount: 0,
              message: null,
              revisionId: "r1-deadbeefcafe",
            },
          ],
          revisionId: "r1-deadbeefcafe",
        },
      }),
    });
  });

  await page.route("**/v1/internal/gateway/credential-pool-refill", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        refill: {
          enabled: true,
          streamKey: "gateway:credential-refill:e2e",
          notificationIntervalSeconds: 60,
          defaultLeaseSeconds: 120,
          maxLeaseSeconds: 600,
          revisionId: "r1-deadbeefcafe",
          providers: [
            {
              providerId: "managed-provider",
              providerLabel: "managed-provider",
              targetSize: 30,
              credentialCount: 1,
              activeCredentialCount: 1,
              deficit: 29,
              needsRefill: true,
              autoRefillEnabled: false,
              directDriverConfigured: false,
              notificationEnabled: true,
              inquiryEnabled: true,
              userRequestEnabled: true,
              outstandingTaskId: null,
              outstandingTaskState: null,
              notificationApi:
                "/v1/internal/gateway/credential-pool-refill/providers/managed-provider/tasks/claim",
              inquiryApi:
                "/v1/internal/gateway/credential-pool-refill/providers/managed-provider",
              credentialStoragePath: "C:\\Gateway\\credentials\\managed-provider",
              storagePasswordConfigured: true,
              archiveStoragePath: "C:\\Gateway\\credentials\\_archive\\managed-provider",
              archivedCredentialCount: 4,
              permanentDeleteEnabled: false,
              revisionId: "r1-deadbeefcafe",
            },
          ],
          recentTasks: [],
        },
      }),
    });
  });

  return state;
}
