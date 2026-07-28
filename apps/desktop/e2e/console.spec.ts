import { expect, test, type Page } from "@playwright/test";
import type {
  ConsoleRouteConfigCommitRequest,
  ConsoleRouteConfigResponse,
  ConsoleRouteRevisionDetailResponse,
  ConsoleRouteRevisionListResponse,
  ManagementSession,
} from "../src/api/contracts";

type MockConsoleState = {
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
      secrets: [{ path: "/providers/0/api_key", configured: true, preview: "sk-***" }],
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
      secrets: [{ path: "/providers/0/api_key", configured: true, preview: "sk-***" }],
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

async function installConsoleApiMocks(page: Page, bootstrapRequired = false): Promise<MockConsoleState> {
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

  return state;
}

test.describe("Gateway web console", () => {
  test("bootstraps the administrator flow in a real browser", async ({ page }) => {
    const state = await installConsoleApiMocks(page, true);

    await page.goto("/ui/");

    await expect(page.getByRole("heading", { name: "初始化管理员" })).toBeVisible();
    await page.getByLabel(/^管理密钥$/).fill("gateway-admin-token");
    await page.getByLabel("确认管理密钥").fill("gateway-admin-token");
    await page.getByRole("button", { name: "创建管理员" }).click();

    await expect(page.getByRole("heading", { name: "Gateway 网页控制台" })).toBeVisible();
    await expect.poll(() => state.bootstrappedTokens).toEqual(["gateway-admin-token"]);
    await expect.poll(() => state.verifiedTokens).toEqual(["gateway-admin-token"]);
  });

  test("signs in and saves a provider route update through the browser console", async ({ page }) => {
    const state = await installConsoleApiMocks(page);

    await page.goto("/ui/");

    await expect(page.getByRole("heading", { name: "登录" })).toBeVisible();
    await page.getByLabel("管理密钥").fill("gateway-admin-token");
    await page.getByRole("button", { name: "登录" }).click();

    await expect(page.getByRole("heading", { name: "Gateway 网页控制台" })).toBeVisible();
    await page
      .getByRole("navigation", { name: "Gateway console navigation" })
      .getByRole("button", { name: /路由编辑/ })
      .click();
    await page.getByRole("button", { name: "添加 Provider 行" }).click();
    await page.getByLabel("Provider ID 2").fill("backup-provider");
    await page.getByLabel("Provider 预设 2").fill("openai");
    await page.getByLabel("Provider 基础 URL 2").fill("https://api.backup.example.com");
    await page.getByLabel("Provider 支持模型 2").fill("gpt-5.4\ngpt-5.4-mini");

    await page
      .getByRole("navigation", { name: "Gateway console navigation" })
      .getByRole("button", { name: /高级 JSON/ })
      .click();
    await expect(page.getByLabel("路由配置 JSON")).toContainText('"backup-provider"');
    await expect(page.getByLabel("路由配置 JSON")).toContainText(
      '"supported_models": [',
    );

    await page.getByRole("button", { name: "保存路由配置" }).click();

    await expect(
      page.getByRole("status", { name: "Gateway console last action" }),
    ).toContainText("已将路由配置保存为激活修订 r2-beadfeedcafe。");
    await expect(
      page.getByLabel("Gateway console summary").getByText("r2-beadfeedcafe"),
    ).toBeVisible();
    await page
      .getByRole("navigation", { name: "Gateway console navigation" })
      .getByRole("button", { name: /Provider 资源/ })
      .click();
    await expect(
      page
        .locator("article")
        .filter({ has: page.getByRole("heading", { name: "Provider 概览" }) })
        .getByText("backup-provider", { exact: true }),
    ).toBeVisible();

    await expect.poll(() => state.lastCommitRequest?.document.providers).toEqual([
      {
        id: "managed-provider",
        preset: "openai",
        base_url: "https://api.primary.example.com",
        supported_models: ["gpt-5.4"],
      },
      {
        id: "backup-provider",
        preset: "openai",
        base_url: "https://api.backup.example.com",
        supported_models: ["gpt-5.4", "gpt-5.4-mini"],
      },
    ]);
  });

  test("keeps the workspace scrollable at a tablet viewport", async ({ page }) => {
    await installConsoleApiMocks(page);
    await page.setViewportSize({ width: 1024, height: 720 });

    await page.goto("/ui/");
    await page.getByLabel("管理密钥").fill("gateway-admin-token");
    await page.getByRole("button", { name: "登录" }).click();
    await expect(page.getByRole("heading", { name: "Gateway 网页控制台" })).toBeVisible();

    const stage = page.locator(".nt-stage");
    await expect.poll(async () => stage.evaluate((element) => element.clientHeight)).toBeGreaterThan(120);
    await stage.hover();
    await page.mouse.wheel(0, 600);
    await expect.poll(async () => stage.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
  });

  test("inspects and restores an archived revision through the browser console", async ({ page }) => {
    const state = await installConsoleApiMocks(page);

    await page.goto("/ui/");

    await expect(page.getByRole("heading", { name: "登录" })).toBeVisible();
    await page.getByLabel("管理密钥").fill("gateway-admin-token");
    await page.getByRole("button", { name: "登录" }).click();

    await expect(page.getByRole("heading", { name: "Gateway 网页控制台" })).toBeVisible();
    await page.getByRole("button", { name: "查看修订 r0-cafebabefeed" }).click();

    await expect(
      page.getByLabel("Selected revision detail").getByText("legacy-provider", { exact: true }),
    ).toBeVisible();
    await expect(page.getByText("answer: gpt-5.4 -> gpt-4.1")).toBeVisible();

    await page.getByRole("button", { name: "恢复为激活配置" }).click();

    await expect(page.getByRole("dialog", { name: "确认恢复修订" })).toBeVisible();
    await expect(page.getByRole("dialog", { name: "确认恢复修订" })).toContainText(
      "r0-cafebabefeed",
    );

    await page.getByRole("button", { name: "确认恢复" }).click();

    await expect(
      page.getByRole("status", { name: "Gateway console last action" }),
    ).toContainText("已将修订 r0-cafebabefeed 恢复为激活修订 r2-beadfeedcafe。");
    const activeRouteSnapshot = page.getByLabel("Active route document snapshot");
    await expect(activeRouteSnapshot).toContainText('"id": "legacy-provider"');
    await expect(activeRouteSnapshot).toContainText('"answer": "gpt-4.1"');

    await expect.poll(() => state.lastCommitRequest?.message).toBe(
      "恢复修订 r0-cafebabefeed",
    );
  });
});
