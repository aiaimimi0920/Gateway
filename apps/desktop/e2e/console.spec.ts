import { expect, test } from "@playwright/test";
import { installConsoleApiMocks } from "./console.e2e.mocks";

test.describe("Gateway web console", () => {
  test("bootstraps the administrator flow in a real browser", async ({ page }) => {
    const state = await installConsoleApiMocks(page, true);

    await page.goto("/ui/");

    await expect(page.getByRole("heading", { name: "初始化管理员" })).toBeVisible();
    await page.getByLabel(/^管理密钥$/).fill("gateway-admin-token");
    await page.getByLabel("确认管理密钥").fill("gateway-admin-token");
    await page.getByRole("button", { name: "创建管理员" }).click();

    await expect(
      page.getByRole("navigation", { name: "Gateway console navigation" }),
    ).toBeVisible();
    await expect.poll(() => state.bootstrappedTokens).toEqual(["gateway-admin-token"]);
    await expect.poll(() => state.verifiedTokens).toEqual(["gateway-admin-token"]);
  });

  test("shows only the three workspace tabs plus a settings button in the rail", async ({
    page,
  }) => {
    await installConsoleApiMocks(page);

    await page.goto("/ui/");
    await page.getByLabel("管理密钥").fill("gateway-admin-token");
    await page.getByRole("button", { name: "登录" }).click();

    const rail = page.getByRole("navigation", { name: "Gateway console navigation" });
    await expect(rail).toBeVisible();
    await expect(page.locator(".nt-rail .nt-brand__name")).toHaveText("Gateway");
    await expect(page.locator(".nt-rail .nt-brand__copy")).toHaveCount(0);

    for (const kept of [/Provider 资源/, /凭据池/, /权益组/]) {
      await expect(rail.getByRole("button", { name: kept })).toBeVisible();
    }
    for (const removed of [/总览/, /路由编辑/, /敏感信息/, /修订历史/, /高级 JSON/]) {
      await expect(rail.getByRole("button", { name: removed })).toHaveCount(0);
    }

    const utility = page.getByRole("navigation", { name: /辅助导航/ });
    await expect(utility.locator("xpath=..")).toHaveClass(/nt-rail__utility/);
    await utility.getByRole("button", { name: /设置/ }).click();
    await expect(page.getByRole("heading", { name: "主题", exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "界面语言" })).toBeVisible();
  });
});
